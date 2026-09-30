//! **HTR-10 against an equal-power LWR**, through one dispersion and dose
//! chain (GitHub #450, #452).
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Not a source
//! > term, dose, siting or licensing argument for HTR-10, NuScale or any plant.
//! > The arms are published design-basis, risk-study and bounding source terms
//! > with **different accident physics** (#450). What is held identical is
//! > everything downstream of the source term, so the comparison isolates the
//! > source term.
//!
//! # Framing (maintainer decisions, 2026-09-30, #450)
//!
//! - **PRIMARY: design basis against design basis**, paired by initiating
//!   event and design class. The maintainer: "design basis against design
//!   basis is more applicable". ~~A like-for-like in-containment comparison~~
//!   is like-for-like in containment, but not in response to LOFC or LOCA.
//! - **SECONDARY: beyond-design-basis bounding**: the HTR-10 KORA air-ingress
//!   bound (a bounding case, not a transient, #420) against WASH-1400 PWR 8.
//!
//! # Methodology
//!
//! Every arm comes from `sembawang::lwr_comparison`, the library the
//! `htgr_sim_v1` map also calls (#453). `bounding_comparison` is asserted to
//! reproduce this example's rows exactly.
//!
//! | Arm | Boundary | Source |
//! |---|---|---|
//! | **HTR-10 DBA: depressurisation** (and water ingress) | environment | Liu & Cao (2002) **Table 8**, published (`htr10_dba_release`); cross-checked against their Table 9 |
//! | **LWR DBA: MHA LOCA**, no removal | environment via containment leakage | RG 1.183 Rev. 1 Table 2 fractions, Table 5 timing, iodine 95 % CsI / 4.85 % elemental / 0.15 % organic; NuScale **`L_a` = 0.20 wt%/day** at P_a (NRC Phase 4 SER Ch. 6, PDF p. 91) for 24 h, then 50 % (PWR, App. A-2.7) |
//! | **LWR DBA: MHA LOCA**, natural deposition only | same | RG 1.183 Rev. 1 App. A-2.2: SRP 6.5.2 (or NUREG/CR-6189 case by case). **Pending literature**: neither document nor NuScale's CNV surface area is held; the rates are an explicit input, never defaulted |
//! | HTR-10 KORA bound | environment (no building credit, #409) | `htr10_air_ingress_bound`: 1400 °C / 140 h, KORA f_ox, TRISO-ATOPS + Liu & Cao circulating at 100 % |
//! | WASH-1400 PWR 8 | atmosphere | Table 5-1 (p. 78): beyond design basis, containment not isolated, no melt |
//! | NUREG-1465 Table 3.13, RG 1.183 Table 2 | **containment** (Bq, context) | into containment only |
//!
//! - **LWR inventory:** NuScale DCA Part 3 Rev. 4 Table B-5 (one module) x
//!   10/160 (160 MWt is the maintainer's attribution; see the reference CSV).
//! - **Containment scaled DOWN with power** (maintainer decision; an
//!   ASSUMPTION): geometric similarity, `V ∝ P` (6,000 ft^3 minimum free
//!   volume, SER PDF pp. 19-20, gives 10.62 m^3 at 10 MWth), `S ∝ V^(2/3)`,
//!   so `S/V` is x 2.520. `L_a`, a fraction per day, is unchanged, so the
//!   no-removal arm does not depend on the scaling; natural deposition
//!   (`∝ S/V`) would be multiplied by 2.520.
//! - **Dose, identical for every arm:** `buangkok` single-plume Gaussian,
//!   **ground-level release** for every arm, ground-level centreline, 1 m/s,
//!   worst stability class at each distance, the whole release passing one
//!   receptor; FGR-15 submersion and groundshine (96 h), FGR-11 inhalation
//!   (max over classes), adult.
//!
//! # Results
//!
//! Printed by `cargo run --release -p sembawang --example lwr_nureg1465_counterpart`,
//! 2026-09-30; recorded on GitHub #452. Deterministic. Not reviewed by a
//! human. Maximum dose over 96 h, class F at every distance \[mSv\]:
//!
//! **Primary, DBA vs DBA** (per MWth: divide by 10):
//!
//! | x \[m\] | HTR-10 depressurisation | HTR-10 water ingress | LWR MHA LOCA, no removal | LWR, natural deposition |
//! |---:|---:|---:|---:|---:|
//! | 400 | 4.19e-3 | 7.59e-3 | 301 | pending literature |
//! | 1000 | 9.08e-4 | 1.64e-3 | 65.2 | pending literature |
//! | 10000 | 3.42e-5 | 6.20e-5 | 2.46 | pending literature |
//!
//! - LWR / HTR-10 depressurisation = **7.2e4** at every distance. It is
//!   distance-independent because every pathway is linear in chi/Q and class F
//!   is the worst at every distance.
//! - Release fractions to the environment (released / core inventory):
//!   noble gases 7.5e-7 (HTR) vs 2.2e-3 (LWR); halogens 9.5e-9 vs 4.3e-4;
//!   alkali metals 1.3e-7 vs 1.1e-3.
//! - The LWR no-removal arm credits nothing RG 1.183 would allow, so it is
//!   an upper value for the RG method. With natural deposition it will be
//!   lower by an amount that is pending literature.
//!
//! **Secondary, beyond-design-basis bounding:**
//!
//! | x \[m\] | HTR-10 KORA bound | WASH-1400 PWR 8 |
//! |---:|---:|---:|
//! | 400 | 66.1 | 58.3 |
//! | 1000 | 14.3 | 12.6 |
//! | 10000 | 0.540 | 0.476 |
//!
//! Within 15 % at every distance, through different groups: HTR more iodine,
//! Sr and Ag; PWR 8 more noble gas and Cs.
//!
//! **All doses are lower bounds** where the FGR tables lack a nuclide (#456).
//! Share of released Bq affected: HTR-10 depressurisation 24.4 % (H-3,
//! C-14), LWR MHA LOCA 28.4 %, KORA bound 0 %, PWR 8 3.5 %.
//!
//! **HTR-10 DBA cross-check against Liu & Cao Table 9 "whole-body"** (their
//! 40 m stack, their unpublished weather). Ours/Table 9 is 0.12 / 0.08 at
//! 250 m, 0.01-0.04 over 0.75-15 km, and 1.46 / 1.13 at 75 km. This is the
//! gap buangkok's #379 test found (Table 9's unstated integration period and
//! weather), made larger by the 96 h groundshine window. Not tuned.
//!
//! **Defects found and fixed on the way (2026-09-30):** negative leakage
//! through catastrophic cancellation (now pinned by a test); Ba-137m
//! double-counted next to buangkok's Cs-137 progeny correction (now skipped
//! when Cs-137 is present).

use changi::activity::inventory::htr10_equilibrium_core;
use sembawang::htr10::Htr10Geometry;
use sembawang::lwr_comparison::{
    bounding_comparison, htr10_air_ingress_bound, htr10_dba_release, htr10_dba_vs_table9,
    incomplete_share, max_dose, nureg1465_pwr_into_containment, nuscale_mha_loca,
    pwr_inventory_scaled, rg1183_containment_leakage, rg1183_pwr_into_containment,
    scaled_containment, wash1400_pwr8_to_atmosphere, AccidentCase, DoseAssumptions, Group,
    N1465Phases, NaturalDeposition, Releases, NATURAL_DEPOSITION_PENDING,
    NUSCALE_LA_PERCENT_PER_DAY,
};
use uom::si::f64::Time;
use uom::si::radioactivity::becquerel;
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
                .sum::<f64>()
                + 0.0 // the empty sum is -0.0; print it as 0
        })
        .collect()
}

/// Released / inventory by group, over nuclides present in both.
fn fractions_by_group(released: &Releases, inventory: &Releases) -> Vec<Option<f64>> {
    Group::ALL
        .iter()
        .map(|g| {
            let (mut r, mut i) = (0.0, 0.0);
            for (n, b) in released.iter().filter(|(n, _)| Group::of(n) == *g) {
                if let Some((_, inv)) = inventory.iter().find(|(m, _)| m == n) {
                    r += b;
                    i += inv;
                }
            }
            (i > 0.0).then(|| r / i)
        })
        .collect()
}

fn frac_text(f: Option<f64>) -> String {
    f.map_or("--".to_string(), |v| format!("{v:.3e}"))
}

fn main() {
    println!("==================================================================");
    println!(" HTR-10 vs an equal-power LWR (10 MWth), one dispersion/dose chain");
    println!(" RESEARCH, EDUCATION AND V&V ONLY. Different accident physics.");
    println!("==================================================================");
    let window = Time::new::<hour>(WINDOW_H);
    let a = DoseAssumptions::bounding_example();
    let deposition = NaturalDeposition::PENDING_LITERATURE;

    // ---------------- PRIMARY: design basis against design basis ----------------
    println!("\nPRIMARY COMPARISON: DESIGN BASIS vs DESIGN BASIS (maintainer decision, 2026-09-30, #450)");
    println!("  HTR-10 depressurisation DBA (Liu & Cao 2002 Table 8, published) vs LWR MHA LOCA");
    println!(
        "  (RG 1.183 Rev. 1 Table 2 / Table 5 / iodine 95-4.85-0.15, NuScale Table B-5 inventory"
    );
    println!(
        "  x {HTR10_MWTH}/160, L_a = {NUSCALE_LA_PERCENT_PER_DAY} %/day (NRC Phase 4 SER Ch. 6, PDF p. 91) for 24 h, then half)."
    );
    let (v, sv) = scaled_containment(HTR10_MWTH);
    println!(
        "  Containment scaled with power (ASSUMPTION): V = {v:.2} m^3, S/V x {sv:.3}; L_a unchanged."
    );
    let dep = htr10_dba_release(AccidentCase::Depressurization);
    let wat = htr10_dba_release(AccidentCase::WaterIngress);
    let lwr = nuscale_mha_loca(HTR10_MWTH, window, &deposition);
    let htr_inv: Releases = htr10_equilibrium_core()
        .iter()
        .map(|e| (e.nuclide.to_string(), e.activity.get::<becquerel>()))
        .collect();
    let inv = pwr_inventory_scaled(HTR10_MWTH);

    println!("\n-- P1. release fraction to the environment (released / core inventory), by group");
    println!(
        "   {:<16}{:>16}{:>16}{:>18}{:>22}",
        "group", "HTR depress.", "HTR water ingr.", "LWR no removal", "LWR nat. deposition"
    );
    let (fd, fw, fl) = (
        fractions_by_group(&dep, &htr_inv),
        fractions_by_group(&wat, &htr_inv),
        fractions_by_group(&lwr.no_removal, &inv),
    );
    let fn_ = lwr
        .natural_deposition
        .as_ref()
        .map(|r| fractions_by_group(r, &inv));
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>16}{:>16}{:>18}{:>22}",
            g.label(),
            frac_text(fd[i]),
            frac_text(fw[i]),
            frac_text(fl[i]),
            fn_.as_ref()
                .map_or("pending".to_string(), |f| frac_text(f[i]))
        );
    }
    println!(
        "   HTR fractions use Liu & Cao Table 1 (changi); '--' = no nuclide of that group in both."
    );
    println!("   LWR natural deposition: {NATURAL_DEPOSITION_PENDING}.");

    println!("\n-- P2. MAXIMUM dose [mSv] vs distance, first {WINDOW_H} h (and per MWth: / {HTR10_MWTH})");
    println!(
        "   {:>8}  {:>14}{:>14}{:>16}{:>14}{:>12}",
        "x [m]", "HTR depress.", "HTR water", "LWR no-remov.", "LWR nat.dep.", "LWR/HTR"
    );
    for x in SWEEP_M {
        let (hd, hw, l) = (
            max_dose(&dep, x, a).total_sv,
            max_dose(&wat, x, a).total_sv,
            max_dose(&lwr.no_removal, x, a).total_sv,
        );
        let ln = lwr
            .natural_deposition
            .as_ref()
            .map_or("pending".to_string(), |r| {
                format!("{:.4e}", 1e3 * max_dose(r, x, a).total_sv)
            });
        println!(
            "   {x:>8.0}  {:>14.4e}{:>14.4e}{:>16.4e}{:>14}{:>12.1}",
            1e3 * hd,
            1e3 * hw,
            1e3 * l,
            ln,
            l / hd
        );
    }
    println!(
        "   FGR-incomplete share of released Bq (LOWER BOUND where > 0): HTR depress. {:.3}, \
         HTR water {:.3}, LWR {:.3} (#456)",
        incomplete_share(&dep),
        incomplete_share(&wat),
        incomplete_share(&lwr.no_removal)
    );

    println!(
        "\n-- P3. HTR-10 DBA cross-check against Liu & Cao Table 9 'whole-body' (their 40 m stack,"
    );
    println!("   their unpublished weather; a DIFFERENT calculation -- the ratio is a finding, not tuned):");
    println!(
        "   {:>8}  {:>14}{:>14}{:>10}   {:>14}{:>14}{:>10}",
        "x [m]", "depr. ours", "Table 9", "ratio", "water ours", "Table 9", "ratio"
    );
    let (cd, cw) = (
        htr10_dba_vs_table9(AccidentCase::Depressurization),
        htr10_dba_vs_table9(AccidentCase::WaterIngress),
    );
    for (d, w) in cd.iter().zip(&cw) {
        println!(
            "   {:>8.0}  {:>14.3e}{:>14.2e}{:>10.2}   {:>14.3e}{:>14.2e}{:>10.2}",
            d.distance_m,
            d.ours_msv,
            d.published_whole_body_msv,
            d.ratio,
            w.ours_msv,
            w.published_whole_body_msv,
            w.ratio
        );
    }

    // ------------- SECONDARY: beyond-design-basis bounding comparison -------------
    println!("\nSECONDARY: BEYOND-DESIGN-BASIS BOUNDING COMPARISON");
    println!(
        "  HTR-10 KORA air-ingress bound (a bounding case, not a transient, #420) vs WASH-1400"
    );
    println!("  PWR 8 (beyond design basis, containment not isolated, no melt).");
    let htr = htr10_air_ingress_bound(htr10_geometry(), window).expect("HTR-10 bound chain");
    let w8 = wash1400_pwr8_to_atmosphere(&inv);
    let n1465_all = nureg1465_pwr_into_containment(&inv, N1465Phases::All);
    let n1465_ge = nureg1465_pwr_into_containment(&inv, N1465Phases::GapAndEarlyInVessel);
    let rg_cont = rg1183_pwr_into_containment(&inv);

    println!(
        "\n-- S1. activity [Bq] by group (containment columns: INTO containment, context only)"
    );
    println!(
        "   {:<16}{:>14}{:>14}{:>16}{:>16}{:>16}",
        "group", "HTR-10 bound", "WASH PWR8", "N1465 cont all", "N1465 cont g+e", "RG cont g+e"
    );
    let cols = [
        by_group(&htr),
        by_group(&w8),
        by_group(&n1465_all),
        by_group(&n1465_ge),
        by_group(&rg_cont),
    ];
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>14.3e}{:>14.3e}{:>16.3e}{:>16.3e}{:>16.3e}",
            g.label(),
            cols[0][i],
            cols[1][i],
            cols[2][i],
            cols[3][i],
            cols[4][i]
        );
    }

    println!("\n-- S2. MAXIMUM dose [mSv] vs distance, first {WINDOW_H} h");
    println!(
        "   {:>8}  {:>14}{:>14}",
        "x [m]", "HTR-10 bound", "WASH PWR 8"
    );
    for x in SWEEP_M {
        let dh = max_dose(&htr, x, a);
        println!(
            "   {x:>8.0}  {:>14.3}{:>14.3}   (class {:?})",
            1e3 * dh.total_sv,
            1e3 * max_dose(&w8, x, a).total_sv,
            dh.class
        );
    }
    println!("\n-- S3. dose [mSv] by group at 400 m");
    let d = [max_dose(&htr, 400.0, a), max_dose(&w8, 400.0, a)];
    println!(
        "   {:<16}{:>14}{:>14}",
        "group", "HTR-10 bound", "WASH PWR 8"
    );
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>14.3}{:>14.3}",
            g.label(),
            1e3 * d[0].by_group[i].1,
            1e3 * d[1].by_group[i].1
        );
    }
    println!(
        "   FGR-incomplete share: HTR-10 bound {:.3}, WASH PWR 8 {:.3} (#456)",
        incomplete_share(&htr),
        incomplete_share(&w8)
    );

    // htgr_sim_v1's map (#453) reads `bounding_comparison`: it must give these
    // rows exactly, or the map and this example have drifted apart.
    let cmp = bounding_comparison(htr10_geometry(), window, HTR10_MWTH, &SWEEP_M, &deposition)
        .expect("comparison");
    for (row, x) in cmp.rows.iter().zip(SWEEP_M) {
        assert_eq!(
            row.htr10_dba_depressurisation_sv,
            max_dose(&dep, x, a).total_sv
        );
        assert_eq!(
            row.lwr_dba_no_removal_sv,
            max_dose(&lwr.no_removal, x, a).total_sv
        );
        assert_eq!(row.lwr_dba_natural_deposition_sv, None);
        assert_eq!(row.htr10_bound_sv, max_dose(&htr, x, a).total_sv);
        assert_eq!(row.wash1400_pwr8_sv, max_dose(&w8, x, a).total_sv);
    }
    println!(
        "\n   (`bounding_comparison`, which htgr_sim_v1's map calls, gives these rows exactly)"
    );

    // Supplementary: RG 1.183 per unit leak rate, for another plant's L_a.
    let per = rg1183_containment_leakage(&inv, None, window).per_percent_per_day;
    println!(
        "\n-- Supplement: LWR MHA LOCA per 1 %/day of L_a (no removal), for another L_a: \
         {:.4e} mSv at 400 m (x {NUSCALE_LA_PERCENT_PER_DAY} = {:.4e}; the L_a arm above is {:.4e},",
        1e3 * max_dose(&per, 400.0, a).total_sv,
        1e3 * NUSCALE_LA_PERCENT_PER_DAY * max_dose(&per, 400.0, a).total_sv,
        1e3 * max_dose(&lwr.no_removal, 400.0, a).total_sv
    );
    println!("   the small-leak linear limit agreeing with the exact run).");
}
