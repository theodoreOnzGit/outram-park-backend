//! HTR-10 air ingress, **bounding case B**: a 1400 °C hold, with the KORA
//! measured SiC-oxidation failure added to boon-lay fuel failure, then released
//! through TRISO-ATOPS (GitHub #434, #435, #438).
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). This is not a
//! > source term for HTR-10 or for any facility, and it computes no dose.
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
//! 1. **The primary-circuit pools start empty** (`zero_pools`). So the
//!    TRISO-ATOPS release carries no circulating activity and no plate-out
//!    lift-off, and under-predicts. The example therefore prints Liu & Cao
//!    Table 3's circulating activity **beside** it, released at 100 % as in
//!    #435, and a combined total.
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
//! | boon-lay increment Δφ_BL, 1400 °C for 140 h (T_B 776 °C stand-in) | **3.768e-7**: φ₁ 3.768e-7, φ₂ 0, end-of-irradiation φ₁ 1.2e-12 |
//! | full-failure fraction | **2.0004e-3** = 3e-4 + 5e-4 + 3.768e-7 + 1.2e-3 |
//! | share carried by KORA f_ox | **60.0 %** (boon-lay's is negligible, 0.02 %) |
//! | SiC-only fraction (stand-ins) | 1.36e-4 |
//! | TRISO-ATOPS release, total over released nuclides | **1.998e13 Bq**. Upstream venting and `FullFlowThrough` agree exactly |
//! | released / core, noble gases and iodine | **2.29e-4**, identical for every noble gas and iodine (one TRISO-ATOPS transport bucket; not decomposed here) |
//! | released / core, Cs-134, Cs-137 | 1.86e-3 (all six fractions; metals include the SiC-only class) |
//! | released / core, Sr-89, Sr-90 | 2.32e-5 |
//! | released / core, **Ag-110m** | **5.57e-2** (breakthrough through **intact** SiC dominates) |
//! | Circulating activity (Liu & Cao Table 3), added at 100 % | 4.551e9 Bq, negligible beside the fuel release |
//! | Screened out (t½ < 5.6 h) | Kr-83m, Kr-85m, Kr-87, Kr-88, I-132, I-134 |
//! | Not in TRISO-ATOPS's table | H-3, Xe-135m, Rb-88 |
//! | Caveats | `negative_atom_count_seen` = true: the known silver floor, so Ag-110m is slightly **over**-stated (see `Caveats`) |
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
use boon_lay::triso_atops_fork::accident::AccidentFractions;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use changi::activity::inventory::htr10_equilibrium_core;
use changi::activity::primary_helium::htr10_primary_helium_end_of_life;
use sembawang::accident::release::{accident_release, accident_release_with_venting, Venting};
use sembawang::htr10::{self, Htr10Geometry};
use sembawang::inventory::{CoreInventory, NuclideInventory};
use sembawang::scenario::TemperatureTransient;
use uom::si::f64::{ThermodynamicTemperature, Time};
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

/// Time samples in the flat transient.
const SAMPLES: usize = 141;
/// Integration steps for the boon-lay isothermal hold.
const BL_STEPS: usize = 200;
/// A single node: a uniform hold has no gradient to resolve.
const N_RADIAL: usize = 1;
const N_AXIAL: usize = 1;

/// Nuclides to show first, the five `htgr_sim_v1` tracks.
const HEADLINE: [&str; 5] = ["Kr-85", "Xe-133", "I-131", "Cs-137", "Ag-110m"];

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

/// A flat `HOLD_CELSIUS` history over `HOLD_HOURS`, uniform over the nodes.
fn flat_hold() -> TemperatureTransient {
    let times: Vec<Time> = (0..SAMPLES)
        .map(|i| Time::new::<hour>(HOLD_HOURS * i as f64 / (SAMPLES - 1) as f64))
        .collect();
    let t = ThermodynamicTemperature::new::<degree_celsius>(HOLD_CELSIUS);
    let temperatures = vec![vec![vec![t; N_AXIAL]; SAMPLES]; N_RADIAL];
    TemperatureTransient::from_nodes(times, temperatures).expect("a flat hold has enough samples")
}

fn main() {
    println!("================================================================");
    println!(" HTR-10 air ingress, BOUNDING CASE B: 1400 C hold, 140 h");
    println!(" boon-lay fuel failure + KORA f_ox -> TRISO-ATOPS (sembawang)");
    println!(" RESEARCH, EDUCATION AND V&V ONLY. Not a source term. No dose.");
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

    println!("-- 3. release [Bq]: TRISO-ATOPS (empty pools) + circulating (100 %)");
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
    println!("   screened out (t1/2 < 4 % of {HOLD_HOURS} h): {:?}", out.screened_out);
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
    println!("   pools start EMPTY (zero_pools): no plate-out lift-off; circulating added by hand above.");
    println!(
        "   venting: Upstream (uniform-constant branch, frac = 1) total {total_atops:.4e} Bq; \
         FullFlowThrough total {ff_total:.4e} Bq (must agree)"
    );
}
