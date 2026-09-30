//! **HTR-10 bounding air ingress vs an equivalent-power LWR**, through one
//! dispersion and dose chain (GitHub #450, #452).
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Not a source
//! > term, dose, siting or licensing argument for HTR-10, NuScale or any plant.
//! > The HTR-10 arm is a **bounding case, not a transient** (#420). The LWR arms
//! > are published design / risk-study source terms with **different accident
//! > physics** (#450). What is held identical is everything downstream of the
//! > source term, so the comparison isolates the source term.
//!
//! # Methodology
//!
//! Every arm comes from `sembawang::lwr_comparison`, the library the
//! `htgr_sim_v1` map also calls (#453):
//!
//! | Arm | Boundary | Source |
//! |---|---|---|
//! | **HTR-10 bounding air ingress** | environment (no building credit, #409) | `htr10_air_ingress_bound`: 1400 °C / 140 h, KORA f_ox, TRISO-ATOPS over 96 h + Liu & Cao circulating at 100 % (the chain of `htr10_air_ingress_kora_bound.rs`) |
//! | **WASH-1400 PWR 8** | atmosphere | Table 5-1 (p. 78): gap release, containment NOT isolated, no melt -- the closest analogue to the HTR-10 bound (#451). A risk-study category, not a design basis |
//! | **RG 1.183 Rev. 1 MHA LOCA** | environment, via containment leakage | Table 2 (p. 20) into containment, Table 5 (p. 24) timing, App. A-2.7 leak `L_a` 24 h then `L_a/2`; **no removal credit**. `L_a` is plant-specific and **not in RG 1.183**: set `LA_PERCENT_PER_DAY` to a sourced value, otherwise the arm is reported **per 1 %/day** |
//! | NUREG-1465 Table 3.13 PWR | **containment** (Bq only; no leak path) | all phases, and gap + early in-vessel |
//! | RG 1.183 Table 2 PWR | **containment** (Bq only) | gap + early in-vessel |
//!
//! LWR inventory: NuScale DCA Part 3 Rev. 4 Table B-5 (one module) x 10/160
//! (160 MWt the maintainer's attribution; see the reference CSV).
//!
//! Dose, identical for every arm (the HTR-10 example's): `buangkok`
//! single-plume Gaussian, **ground-level release** (an LWR containment leak is
//! also conventionally ground-level; kept identical), ground-level centreline,
//! 1 m/s, worst stability class at each distance, the whole release passing one
//! receptor; FGR-15 submersion and groundshine (96 h), FGR-11 inhalation
//! (max over classes), adult. Distances: those of the HTR-10 example.
//!
//! # Results
//!
//! Printed by `cargo run --release -p sembawang --example lwr_nureg1465_counterpart`,
//! 2026-09-30, on the reference CSVs of `crates/sembawang/reference/lwr/`;
//! recorded on GitHub #452. Deterministic (no sampling). Not reviewed by a human.
//!
//! Maximum individual dose, first 96 h, worst class (F at every distance) \[mSv\]:
//!
//! | x \[m\] | HTR-10 bound | WASH-1400 PWR 8 | RG 1.183, **per 1 %/day** `L_a` |
//! |---:|---:|---:|---:|
//! | 400 | 66.1 | 58.3 | 1510 |
//! | 1000 | 14.3 | 12.6 | 327 |
//! | 3000 | 2.67 | 2.36 | 61.0 |
//! | 10000 | 0.540 | 0.476 | 12.3 |
//!
//! At 400 m, by group (HTR / PWR 8 / RG per 1 %/d): halogens 37.9 / 24.3 / 1120;
//! alkali metals 19.7 / 27.4 / 309; Ba/Sr 6.30 / 0.005 / 69.2; noble gases
//! 0.074 / 6.68 / 12.0; Ag 2.19 / 0 / 0.
//!
//! **Interpretation.**
//!
//! - The HTR-10 bounding case and WASH-1400 PWR 8, the two beyond-design-basis,
//!   uncontained, no-melt cases, give doses within 15 % of each other at every
//!   distance (HTR / PWR 8 = 1.13). They reach them through different groups:
//!   the HTR has more iodine, strontium and silver, and the PWR 8 gap release
//!   has more noble gas and caesium.
//! - The RG 1.183 design-basis arm is a release **per 1 %/day** of leak rate.
//!   Its dose is linear in `L_a` in the small-leak limit, so a plant's
//!   Technical Specification value (not in RG 1.183, never assumed here)
//!   multiplies the column. It credits **no** removal (sprays, deposition,
//!   pool, filtration), so it is an upper value for the RG method. It is a
//!   design-basis melt source term into an intact containment: different
//!   accident physics from both other arms (#450).
//! - **The LWR doses are LOWER BOUNDS.** buangkok's FGR tables hold about 24
//!   nuclides (FGR-11 inhalation: 10). Released Bq with a pathway missing, and
//!   therefore counted as zero: HTR-10 0 %, WASH PWR 8 3.5 % (Te, Cs-136, Rb,
//!   Ba-140, Sr-91/92), RG 1.183 28.4 % (adds actinides, lanthanides, Ru, Mo).
//!   The Te-group row is 0 because no Te nuclide has a coefficient, not
//!   because Te gives no dose. Follow-up: #456.
//!
//! **Defects found and fixed on the way (2026-09-30).**
//!
//! - The leakage integrator returned negative Bq for long-lived nuclides
//!   through catastrophic cancellation; now pinned by a test.
//! - NuScale Table B-5 lists Ba-137m, which buangkok's Cs-137 progeny
//!   correction already carries, so it was double-counted; now skipped when
//!   Cs-137 is present.

use sembawang::htr10::Htr10Geometry;
use sembawang::lwr_comparison::{
    htr10_air_ingress_bound, max_dose, nureg1465_pwr_into_containment, pwr_inventory_scaled,
    rg1183_containment_leakage, rg1183_pwr_into_containment, wash1400_pwr8_to_atmosphere,
    DoseAssumptions, Group, N1465Phases, Releases,
};
use uom::si::f64::Time;
use uom::si::time::hour;

/// HTR-10 thermal power \[MWth\] (IAEA-TECDOC-1382), the LWR scaling target.
const HTR10_MWTH: f64 = 10.0;
/// Dose window \[h\], the HTR-10 bounding example's.
const WINDOW_H: f64 = 96.0;
/// Receptor distances \[m\], the HTR-10 bounding example's.
const SWEEP_M: [f64; 9] = [
    400.0, 600.0, 800.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 10000.0,
];

fn htr10_geometry() -> Htr10Geometry {
    let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
    let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
    Htr10Geometry {
        kernel_radius: particle.kernel_radius,
        sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
        graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
    }
}

fn by_group(r: &Releases) -> Vec<f64> {
    Group::ALL
        .iter()
        .map(|g| {
            r.iter()
                .filter(|(n, _)| Group::of(n) == *g)
                .map(|(_, b)| b)
                .sum()
        })
        .collect()
}

fn main() {
    println!("==================================================================");
    println!(" HTR-10 BOUNDING air ingress vs LWR (10 MWth), one dose chain");
    println!(" RESEARCH, EDUCATION AND V&V ONLY. Different accident physics.");
    println!("==================================================================\n");
    let window = Time::new::<hour>(WINDOW_H);
    let la = std::env::var("LA_PERCENT_PER_DAY")
        .ok()
        .and_then(|v| v.parse::<f64>().ok());

    let htr = htr10_air_ingress_bound(htr10_geometry(), window).expect("HTR-10 bound chain");
    let inv = pwr_inventory_scaled(HTR10_MWTH);
    let w8 = wash1400_pwr8_to_atmosphere(&inv);
    let leak = rg1183_containment_leakage(&inv, la, window);
    let rg_env = leak
        .at_la
        .clone()
        .unwrap_or_else(|| leak.per_percent_per_day.clone());
    let rg_label = match la {
        Some(v) => format!("RG1.183 env (L_a {v} %/d)"),
        None => "RG1.183 env PER 1 %/d L_a".to_string(),
    };
    let n1465_all = nureg1465_pwr_into_containment(&inv, N1465Phases::All);
    let n1465_ge = nureg1465_pwr_into_containment(&inv, N1465Phases::GapAndEarlyInVessel);
    let rg_cont = rg1183_pwr_into_containment(&inv);

    println!("-- 1. activity [Bq] by group, 10 MWth");
    println!(
        "   {:<16}{:>14}{:>14}{:>16}{:>16}{:>16}{:>16}",
        "group",
        "HTR-10 bound",
        "WASH PWR8",
        "RG env",
        "N1465 cont all",
        "N1465 cont g+e",
        "RG cont g+e"
    );
    println!(
        "   {:<16}{:>14}{:>14}{:>16}{:>16}{:>16}{:>16}",
        "(boundary)",
        "environment",
        "atmosphere",
        "environment",
        "CONTAINMENT",
        "CONTAINMENT",
        "CONTAINMENT"
    );
    let cols = [
        by_group(&htr),
        by_group(&w8),
        by_group(&rg_env),
        by_group(&n1465_all),
        by_group(&n1465_ge),
        by_group(&rg_cont),
    ];
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>14.3e}{:>14.3e}{:>16.3e}{:>16.3e}{:>16.3e}{:>16.3e}",
            g.label(),
            cols[0][i],
            cols[1][i],
            cols[2][i],
            cols[3][i],
            cols[4][i],
            cols[5][i]
        );
    }
    println!("   RG env column: {rg_label}. HTR-10 has no Te-group or 'other' nuclides in its tracked set.\n");

    let a = DoseAssumptions::bounding_example();
    println!("-- 2. MAXIMUM dose [mSv] vs distance, first {WINDOW_H} h, same site/weather/height/receptors");
    println!(
        "   {:>8}  {:>14}{:>14}{:>24}",
        "x [m]", "HTR-10 bound", "WASH PWR 8", rg_label
    );
    for x in SWEEP_M {
        let dh = max_dose(&htr, x, a);
        let dw = max_dose(&w8, x, a);
        let dr = max_dose(&rg_env, x, a);
        println!(
            "   {x:>8.0}  {:>14.3}{:>14.3}{:>24.3}   (class {:?})",
            1e3 * dh.total_sv,
            1e3 * dw.total_sv,
            1e3 * dr.total_sv,
            dh.class
        );
    }

    println!("\n-- 3. dose [mSv] by group at 400 m");
    let d = [
        max_dose(&htr, 400.0, a),
        max_dose(&w8, 400.0, a),
        max_dose(&rg_env, 400.0, a),
    ];
    println!(
        "   {:<16}{:>14}{:>14}{:>24}",
        "group", "HTR-10 bound", "WASH PWR 8", rg_label
    );
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>14.3}{:>14.3}{:>24.3}",
            g.label(),
            1e3 * d[0].by_group[i].1,
            1e3 * d[1].by_group[i].1,
            1e3 * d[2].by_group[i].1
        );
    }
    println!("\n-- 4. COVERAGE: released Bq whose nuclide lacks a buangkok FGR coefficient on");
    println!("   at least one pathway. Those pathways count ZERO, so the dose is a LOWER BOUND.");
    for (name, rel, dd) in [
        ("HTR-10 bound", &htr, &d[0]),
        ("WASH PWR 8", &w8, &d[1]),
        ("RG 1.183", &rg_env, &d[2]),
    ] {
        let tot: f64 = rel.iter().map(|(_, b)| b).sum();
        let miss: f64 = rel
            .iter()
            .filter(|(n, _)| dd.missing.contains(n))
            .map(|(_, b)| b)
            .sum();
        println!(
            "   {name:<14} {:>6.1} % of released Bq incomplete; nuclides: {:?}",
            100.0 * miss / tot,
            dd.missing
        );
    }
    println!(
        "\n   Boundaries differ: HTR-10 and WASH-1400 are releases to the environment/atmosphere;"
    );
    println!(
        "   the NUREG-1465 and RG 1.183 'containment' columns are INTO CONTAINMENT, not releases."
    );
    println!(
        "   RG 1.183 to the environment needs the plant's TS leak rate L_a (not in RG 1.183);"
    );
    if la.is_none() {
        println!("   none supplied, so that arm is PER 1 %/day of L_a -- scale linearly (small-leak limit).");
    }
    println!("   No containment removal credit is taken (RG 1.183 App. A-2.2..2.6 allow some; none used).");
}
