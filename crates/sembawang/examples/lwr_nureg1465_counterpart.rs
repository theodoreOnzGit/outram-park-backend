//! **HTR-10 against an equal-power LWR**, through one dispersion and dose
//! chain (GitHub #450, #452, #464).
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Not a source
//! > term, dose, siting, emergency-planning or licensing argument for HTR-10,
//! > NuScale or any plant. The arms are published design-basis, risk-study and
//! > bounding source terms with **different accident physics** (#450). What is
//! > held identical is everything downstream of the source term, so the
//! > comparison isolates the source term.
//!
//! # Framing (maintainer decisions, 2026-09-30, #450, #464)
//!
//! ~~**PRIMARY: design basis against design basis** ... **SECONDARY:
//! beyond-design-basis bounding**: the HTR-10 KORA air-ingress bound against
//! WASH-1400 PWR 8.~~ **CHANGED 2026-09-30 (#464): paired by INITIATING EVENT
//! and severity**, DLOFC (HTR) against LOCA (LWR):
//!
//! - **Design basis: DLOFC vs LOCA.** HTR-10 depressurisation (Liu & Cao
//!   Table 8) vs the LWR MHA LOCA (RG 1.183 Rev. 1).
//! - **Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core
//!   melt.** The KORA bound (a bounding case, not a transient, #420) vs a
//!   LOCA with ECCS failure: NUREG-1465 Table 3.13, all phases, into an
//!   **intact** containment leaking at `L_a`.
//! - **Context:** WASH-1400 PWR 8, no-melt, uncontained; paired by
//!   containment state, not initiator.
//!
//! **Crediting basis (maintainer decision, 2026-09-30):** "The comparison
//! neglects the pools because we are comparing technology at the reactor
//! level, not what is surrounding the reactor. If NuScale gives pool credit,
//! then HTGR can be submerged in a pool as well." Each side is credited
//! **only with its reactor-level inherent barrier**: the containment vessel
//! leaking at `L_a` (LWR), the TRISO particles (HTR). Nothing surrounding the
//! reactor is credited on either side: not NuScale's reactor pool, reactor
//! building, sprays or filters, and not the HTR confinement or building
//! (#409) or a hypothetical pool. Pool scrubbing is excluded **by design**,
//! not pending literature. In-containment natural deposition is part of the
//! containment barrier, so that arm stays (pending literature).
//!
//! # Methodology
//!
//! Every arm comes from `sembawang::lwr_comparison`, the library the
//! `htgr_sim_v1` map also calls (#453). `bounding_comparison` is asserted to
//! reproduce this example's rows exactly, and `ARM_COLUMNS` fixes the column
//! order and labels for both.
//!
//! | Tier | Arm | Boundary | Source |
//! |---|---|---|---|
//! | Design basis | **HTR-10 DLOFC: depressurisation** (and water ingress) | environment | Liu & Cao (2002) **Table 8**, published (`htr10_dba_release`); cross-checked against their Table 9 |
//! | Design basis | **LWR LOCA (RG 1.183 MHA)**, no removal | environment via containment leakage | RG 1.183 Rev. 1 Table 2 fractions, Table 5 timing, iodine 95 % CsI / 4.85 % elemental / 0.15 % organic; NuScale **`L_a` = 0.20 wt%/day** at P_a (NRC Phase 4 SER Ch. 6, PDF p. 91) for 24 h, then 50 % (PWR, App. A-2.7) |
//! | Design basis | LWR LOCA, natural deposition only | same | RG 1.183 Rev. 1 App. A-2.2: SRP 6.5.2 (or NUREG/CR-6189 case by case). **Pending literature**; the rates are an explicit input, never defaulted |
//! | Beyond design basis | **HTR-10 DLOFC + air ingress, KORA bound** | environment (no building credit, #409) | `htr10_air_ingress_bound`: 1400 °C / 140 h, KORA f_ox, TRISO-ATOPS + Liu & Cao circulating at 100 % |
//! | Beyond design basis | **LWR LOCA + core melt**, no removal | environment via containment leakage | `nuscale_severe_loca`: **NUREG-1465 Table 3.13 PWR, all four phases** (printed p. 13, PDF p. 22; spot-checked against the PDF text layer and the maintainer's kovan digitisation, every cell), **Table 3.6** timing (printed p. 9, PDF p. 18): gap from 30 s for 0.5 h, early in-vessel 1.3 h to vessel breach at 1.808 h, ex-vessel 2 h and late in-vessel 10 h both from breach (s.3.3: concurrent); linear within each phase (assumed; RG 1.183's default). Same inventory, `L_a`, species and integrator as the DBA arm |
//! | Beyond design basis | LWR LOCA + core melt, natural deposition | same | pending literature, as the DBA arm |
//! | Context | WASH-1400 PWR 8 | atmosphere | Table 5-1 (p. 78): containment not isolated, no melt |
//! | (supporting) | NUREG-1465 Table 3.13, RG 1.183 Table 2 | **containment** (Bq) | into containment only |
//!
//! - **Contained core melt (ASSUMPTION, stated):** the containment stays
//!   intact for the 96 h window: no early failure, no bypass, no basemat
//!   melt-through; NUREG-1465's ex-vessel (core-concrete) release goes into
//!   the same leaking atmosphere. WASH-1400 PWR 1-3 are the core-melt,
//!   containment-FAILURE categories (context only; not run).
//! - **LWR inventory:** NuScale DCA Part 3 Rev. 4 Table B-5 (one module) x
//!   10/160 (160 MWt is the maintainer's attribution; see the reference CSV).
//! - **Containment scaled DOWN with power** (maintainer decision; an
//!   ASSUMPTION): geometric similarity, `V ∝ P` (6,000 ft^3 minimum free
//!   volume, SER PDF pp. 19-20, gives 10.62 m^3 at 10 MWth), `S ∝ V^(2/3)`,
//!   so `S/V` is x 2.520. `L_a`, a fraction per day, is unchanged, so the
//!   no-removal arms do not depend on the scaling; natural deposition
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
//! 2026-09-30; recorded on GitHub #452 and #464. Deterministic. Not reviewed
//! by a human. Maximum dose over 96 h, class F at every distance \[mSv\]
//! (per MWth: divide by 10). **Every LWR dose is a LOWER BOUND** (#456).
//!
//! | x \[m\] | DB: HTR-10 DLOFC | DB: LWR LOCA, no removal | BDB: HTR-10 DLOFC + air ingress (KORA) | BDB: LWR LOCA + core melt, no removal | Context: WASH-1400 PWR 8 |
//! |---:|---:|---:|---:|---:|---:|
//! | 400 | 4.19e-3 | 301 | 66.1 | 941 | 58.3 |
//! | 1000 | 9.08e-4 | 65.2 | 14.3 | 204 | 12.6 |
//! | 10000 | 3.42e-5 | 2.46 | 0.540 | 7.68 | 0.476 |
//!
//! Both natural-deposition columns: pending literature.
//!
//! - **Design basis:** LWR / HTR-10 = **7.2e4** at every distance
//!   (distance-independent: every pathway is linear in chi/Q and class F is
//!   the worst everywhere). Release fractions to the environment: noble
//!   gases 7.5e-7 (HTR) vs 2.2e-3 (LWR); halogens 9.5e-9 vs 4.3e-4; alkali
//!   metals 1.3e-7 vs 1.1e-3.
//! - **Beyond design basis:** HTR-10 bound / LWR core melt = **0.070** at
//!   every distance (the LWR is 14x higher). Release fractions to the
//!   environment, KORA vs LWR core melt: noble gases 1.9e-4 vs 2.4e-3;
//!   halogens 1.9e-4 vs 8.6e-4; alkali metals 1.2e-3 vs 3.6e-3; Ba/Sr
//!   1.9e-5 vs 3.9e-4; Ag 2.5e-2 (HTR only). At 400 m the LWR core-melt dose
//!   is halogens 444, Ba/Sr 299, alkali metals 196, noble gases 2.5 mSv.
//! - **LWR core melt / LWR DBA = 3.13.** Predicted before the run (#464):
//!   higher, by about 2 (range 1.5-3). Direction right; magnitude above the
//!   predicted range, because the Ba/Sr term (Table 3.13 0.12 vs Table 2
//!   0.0054, mostly inhaled Sr) carries 299 mSv at 400 m, which the
//!   prediction under-weighted.
//! - **Lower bounds (#456).** Share of released Bq lacking an FGR coefficient
//!   on some pathway: HTR-10 DLOFC 24.4 % (H-3, C-14), LWR DBA LOCA 28.4 %,
//!   KORA 0 %, LWR core melt 27.6 % (the Te group scores zero dose in both LWR
//!   LOCA arms), WASH-1400 PWR 8 3.5 %.
//! - **HTR-10 DLOFC cross-check against Liu & Cao Table 9 "whole-body"**
//!   (their 40 m stack, their unpublished weather). Ours/Table 9 is 0.12 /
//!   0.08 at 250 m, 0.01-0.04 over 0.75-15 km, and 1.46 / 1.13 at 75 km. This
//!   is the gap buangkok's #379 test found (Table 9's unstated integration
//!   period and weather), made larger by the 96 h groundshine window. Not
//!   tuned.
//!
//! **Numbers corrected 2026-09-30 (#464).** The LWR DBA LOCA was 301.36 /
//! 65.239 / 2.4613 mSv (400 / 1000 / 10 000 m); it is now 301.11 / 65.184 /
//! 2.4592 (-0.083 %). The containment integrator evaluated each phase's source
//! at the 60 s step midpoint. That gave RG 1.183's 798 s gap phase 840 s of
//! source, 5.3 % over its Table 2 fraction. The source is now weighted by the
//! overlap of step and phase, and it is pinned by
//! `dba_source_is_conserved_through_the_leak`. The quoted 3-figure values are
//! unchanged. Ba/Sr DBA release fraction: 1.782e-5 -> 1.758e-5.
//!
//! **Defects found and fixed earlier (2026-09-30):**
//! - Negative leakage through catastrophic cancellation, now pinned by a test.
//! - Ba-137m double-counted next to buangkok's Cs-137 progeny correction. It
//!   is now skipped when Cs-137 is present.

use changi::activity::inventory::htr10_equilibrium_core;
use sembawang::htr10::Htr10Geometry;
use sembawang::lwr_comparison::{
    bounding_comparison, htr10_air_ingress_bound, htr10_dba_release, htr10_dba_vs_table9,
    incomplete_share, max_dose, n1465_timing, nureg1465_pwr_into_containment, nuscale_mha_loca,
    nuscale_severe_loca, pwr_inventory_scaled, rg1183_containment_leakage,
    rg1183_pwr_into_containment, scaled_containment, wash1400_pwr8_to_atmosphere, AccidentCase,
    DoseAssumptions, Group, N1465Phases, NaturalDeposition, Releases, Tier, ARM_COLUMNS,
    CONTAINED_CORE_MELT_ASSUMPTION, NATURAL_DEPOSITION_PENDING, NUSCALE_LA_PERCENT_PER_DAY,
    REACTOR_LEVEL_BASIS,
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
    println!(" Paired by INITIATING EVENT and severity (maintainer decision, 2026-09-30,");
    println!(" #450/#464): DLOFC (HTR) vs LOCA (LWR).");
    println!(" Basis: {REACTOR_LEVEL_BASIS}");
    let window = Time::new::<hour>(WINDOW_H);
    let a = DoseAssumptions::bounding_example();
    let deposition = NaturalDeposition::PENDING_LITERATURE;
    let (v, sv) = scaled_containment(HTR10_MWTH);
    let dep = htr10_dba_release(AccidentCase::Depressurization);
    let wat = htr10_dba_release(AccidentCase::WaterIngress);
    let lwr = nuscale_mha_loca(HTR10_MWTH, window, &deposition);
    let severe = nuscale_severe_loca(HTR10_MWTH, window, &deposition);
    let htr_inv: Releases = htr10_equilibrium_core()
        .iter()
        .map(|e| (e.nuclide.to_string(), e.activity.get::<becquerel>()))
        .collect();
    let inv = pwr_inventory_scaled(HTR10_MWTH);

    // ---------------- TIER 1: design basis, DLOFC vs LOCA ----------------
    println!("\nTIER 1. {}", Tier::DesignBasis.label().to_uppercase());
    println!("  HTR-10 DLOFC: depressurisation (Liu & Cao 2002 Table 8, published) vs LWR LOCA:");
    println!(
        "  RG 1.183 Rev. 1 MHA (Table 2 / Table 5 / iodine 95-4.85-0.15), NuScale Table B-5 inventory"
    );
    println!(
        "  x {HTR10_MWTH}/160, L_a = {NUSCALE_LA_PERCENT_PER_DAY} %/day (NRC Phase 4 SER Ch. 6, PDF p. 91) for 24 h, then half."
    );
    println!(
        "  Containment scaled with power (ASSUMPTION): V = {v:.2} m^3, S/V x {sv:.3}; L_a unchanged."
    );

    println!("\n-- D1. release fraction to the environment (released / core inventory), by group");
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

    println!(
        "\n-- D2. MAXIMUM dose [mSv] vs distance, first {WINDOW_H} h (per MWth: / {HTR10_MWTH})"
    );
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
        "\n-- D3. HTR-10 DLOFC cross-check against Liu & Cao Table 9 'whole-body' (their 40 m stack,"
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

    // ------ TIER 2: beyond design basis, DLOFC + air ingress vs LOCA + core melt ------
    println!(
        "\nTIER 2. {}",
        Tier::BeyondDesignBasis.label().to_uppercase()
    );
    println!(
        "  HTR-10 DLOFC + air ingress: the KORA bound (a bounding case, not a transient, #420) vs"
    );
    println!("  LWR LOCA with ECCS failure -> core melt: NUREG-1465 Table 3.13 PWR, ALL phases");
    println!(
        "  (gap {:.3}->{:.3} h, early in-vessel ->{:.3} h = vessel breach, ex-vessel ->{:.3} h,",
        n1465_timing::GAP_ONSET_H,
        n1465_timing::GAP_ONSET_H + n1465_timing::GAP_H,
        n1465_timing::VESSEL_BREACH_H,
        n1465_timing::VESSEL_BREACH_H + n1465_timing::EX_VESSEL_H
    );
    println!(
        "  late in-vessel ->{:.3} h; Table 3.6), same inventory, containment and L_a as Tier 1.",
        n1465_timing::VESSEL_BREACH_H + n1465_timing::LATE_IN_VESSEL_H
    );
    println!("  ASSUMPTION: {CONTAINED_CORE_MELT_ASSUMPTION}.");
    let htr = htr10_air_ingress_bound(htr10_geometry(), window).expect("HTR-10 bound chain");

    println!("\n-- B1. release fraction to the environment (released / core inventory), by group");
    println!(
        "   {:<16}{:>16}{:>18}{:>22}",
        "group", "HTR KORA bound", "LWR melt no-rem.", "LWR melt nat. dep."
    );
    let (fh, fs) = (
        fractions_by_group(&htr, &htr_inv),
        fractions_by_group(&severe.no_removal, &inv),
    );
    let fsn = severe
        .natural_deposition
        .as_ref()
        .map(|r| fractions_by_group(r, &inv));
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>16}{:>18}{:>22}",
            g.label(),
            frac_text(fh[i]),
            frac_text(fs[i]),
            fsn.as_ref()
                .map_or("pending".to_string(), |f| frac_text(f[i]))
        );
    }

    println!(
        "\n-- B2. MAXIMUM dose [mSv] vs distance, first {WINDOW_H} h (per MWth: / {HTR10_MWTH})"
    );
    println!(
        "   {:>8}  {:>14}{:>16}{:>14}{:>14}{:>16}",
        "x [m]", "HTR KORA", "LWR melt", "LWR nat.dep.", "HTR/LWR melt", "melt/LWR DBA"
    );
    for x in SWEEP_M {
        let (h, s, d) = (
            max_dose(&htr, x, a).total_sv,
            max_dose(&severe.no_removal, x, a).total_sv,
            max_dose(&lwr.no_removal, x, a).total_sv,
        );
        let sn = severe
            .natural_deposition
            .as_ref()
            .map_or("pending".to_string(), |r| {
                format!("{:.4e}", 1e3 * max_dose(r, x, a).total_sv)
            });
        println!(
            "   {x:>8.0}  {:>14.4}{:>16.4}{:>14}{:>14.4}{:>16.3}",
            1e3 * h,
            1e3 * s,
            sn,
            h / s,
            s / d
        );
    }
    println!(
        "   FGR-incomplete share (LOWER BOUND where > 0): HTR KORA {:.3}, LWR melt {:.3} (#456)",
        incomplete_share(&htr),
        incomplete_share(&severe.no_removal)
    );

    // ---------------- CONTEXT: WASH-1400 PWR 8 ----------------
    println!("\nCONTEXT. {}", Tier::Context.label());
    println!("  WASH-1400 Table 5-1 PWR 8: gap release, containment not isolated, no core melt.");
    println!("  (WASH-1400 PWR 1-3 are the core-melt, containment-FAILURE categories.)");
    let w8 = wash1400_pwr8_to_atmosphere(&inv);
    let n1465_all = nureg1465_pwr_into_containment(&inv, N1465Phases::All);
    let n1465_ge = nureg1465_pwr_into_containment(&inv, N1465Phases::GapAndEarlyInVessel);
    let rg_cont = rg1183_pwr_into_containment(&inv);

    println!(
        "\n-- C1. activity [Bq] by group (containment columns: INTO containment, context only)"
    );
    println!(
        "   {:<16}{:>14}{:>14}{:>14}{:>16}{:>16}{:>16}",
        "group",
        "HTR-10 bound",
        "LWR melt env",
        "WASH PWR8",
        "N1465 cont all",
        "N1465 cont g+e",
        "RG cont g+e"
    );
    let cols = [
        by_group(&htr),
        by_group(&severe.no_removal),
        by_group(&w8),
        by_group(&n1465_all),
        by_group(&n1465_ge),
        by_group(&rg_cont),
    ];
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>14.3e}{:>14.3e}{:>14.3e}{:>16.3e}{:>16.3e}{:>16.3e}",
            g.label(),
            cols[0][i],
            cols[1][i],
            cols[2][i],
            cols[3][i],
            cols[4][i],
            cols[5][i]
        );
    }
    println!("\n-- C2. dose [mSv] by group at 400 m");
    let d = [
        max_dose(&htr, 400.0, a),
        max_dose(&severe.no_removal, 400.0, a),
        max_dose(&w8, 400.0, a),
    ];
    println!(
        "   {:<16}{:>14}{:>14}{:>14}",
        "group", "HTR-10 bound", "LWR melt", "WASH PWR 8"
    );
    for (i, g) in Group::ALL.iter().enumerate() {
        println!(
            "   {:<16}{:>14.3}{:>14.3}{:>14.3}",
            g.label(),
            1e3 * d[0].by_group[i].1,
            1e3 * d[1].by_group[i].1,
            1e3 * d[2].by_group[i].1
        );
    }
    println!(
        "   FGR-incomplete share: WASH PWR 8 {:.3} (#456)",
        incomplete_share(&w8)
    );

    // ---------------- ALL ARMS, in tier order: what htgr_sim_v1's map shows ----------------
    // htgr_sim_v1's map (#453) reads `bounding_comparison`: it must give these
    // rows exactly, or the map and this example have drifted apart.
    let cmp = bounding_comparison(htr10_geometry(), window, HTR10_MWTH, &SWEEP_M, &deposition)
        .expect("comparison");
    println!("\nALL ARMS, TIER ORDER (sembawang::lwr_comparison::ARM_COLUMNS) [mSv]:");
    for (i, c) in ARM_COLUMNS.iter().enumerate() {
        println!("   col {}: {}", i + 1, c.heading);
    }
    print!("   {:>8}", "x [m]");
    for i in 0..ARM_COLUMNS.len() {
        print!("{:>12}", format!("col {}", i + 1));
    }
    println!();
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
        assert_eq!(
            row.lwr_severe_loca_no_removal_sv,
            max_dose(&severe.no_removal, x, a).total_sv
        );
        assert_eq!(row.lwr_severe_loca_natural_deposition_sv, None);
        assert_eq!(row.wash1400_pwr8_sv, max_dose(&w8, x, a).total_sv);
        print!("   {:>8.0}", row.distance_m);
        for v in row.arm_doses_sv() {
            print!(
                "{:>12}",
                v.map_or("pending".to_string(), |s| format!("{:.3e}", 1e3 * s))
            );
        }
        println!();
    }
    println!("   (`bounding_comparison`, which htgr_sim_v1's map calls, gives these rows exactly)");

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
