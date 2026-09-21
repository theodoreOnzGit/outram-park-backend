// SPDX-License-Identifier: GPL-3.0

//! End-to-end: a prescribed transient and inventory out to a `changi` source
//! term.
//!
//! # Consistency checks, not verification
//!
//! The release physics underneath is verified code-to-code against upstream
//! TRISO-ATOPS, in `boon-lay`'s own suite. **The orchestration here is not**,
//! and cannot be: upstream's own driver pairs a venting *gather* with a
//! temperature *prefix*, so on any transient where the two differ this crate
//! deliberately does not reproduce it. Comparing against it would be comparing
//! against a defect.
//!
//! So these assert properties that must hold for any correct orchestration —
//! well-formedness, conservation, linearity, and that the caveat channel
//! actually fires — plus one that pins the deliberate divergence.

use changi::activity::deposition::DepositionGroup;
use sembawang::accident::release::{accident_release, PlantParameters};
use sembawang::inventory::CoreInventory;
use sembawang::scenario::TemperatureTransient;
use sembawang::units;
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::radioactivity::becquerel;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

const NUCLIDES: [&str; 5] = ["Kr-85", "Kr-88", "I-131", "Te-132", "Cs-137"];

fn c(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(x)
}

fn plant() -> PlantParameters {
    // The three accident-phase parameters are arguments precisely because the
    // reference case does not supply them. These are a caller's choice for a
    // shakedown run and are NOT cited values.
    PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0)
}

/// A heat-up and hold: 600 C to 1600 C over 1e4 s, held to 1e5 s.
fn heating_transient(samples: usize) -> TemperatureTransient {
    TemperatureTransient::from_ramp(
        c(600.0),
        c(1600.0),
        Time::new::<second>(1.0e4),
        Time::new::<second>(1.0e5),
        samples,
        2,
        3,
    )
    .unwrap()
}

#[test]
fn the_chain_runs_and_produces_a_well_formed_source_term() {
    let inv = CoreInventory::unit(&NUCLIDES, 2, 3);
    let out = accident_release(&inv, &heating_transient(51), &plant()).unwrap();

    // SourceTerm::new validates on construction, so reaching here already
    // means the windows are contiguous and every activity is non-negative.
    let term = &out.source_term;
    assert!(!term.windows.is_empty(), "there should be release windows");
    for n in &term.nuclides {
        assert_eq!(
            n.released.len(),
            term.windows.len(),
            "{} must carry one activity per window",
            n.label
        );
    }

    // Contiguous and ascending -- re-asserted explicitly because it is the
    // property `changi` depends on.
    for pair in term.windows.windows(2) {
        assert_eq!(
            pair[0].end.get::<second>(),
            pair[1].start.get::<second>(),
            "windows must be contiguous"
        );
        assert!(pair[0].duration().get::<second>() > 0.0);
    }

    // The caveat channel must actually fire: upstream always forces the first
    // sample fully vented, so this is set on every run that vents.
    assert!(
        out.caveats.first_sample_forced_fully_vented,
        "upstream hard-codes frac[0] = 1; this must be reported, not hidden"
    );
    assert!(out.caveats.any());
    assert!(!out.caveats.lines().is_empty());

    // A 600 C start is below the ~700 C lower edge of the fitted Arrhenius
    // range, so the clamp caveat must fire too.
    assert!(
        out.caveats.diffusion_coefficient_clamped,
        "a transient starting at 600 C leaves the fitted range and must say so"
    );
}

/// Deposition groups must come from the atomic number, not from `boon-lay`'s
/// transport grouping — Te is the case that distinguishes them.
#[test]
fn tellurium_is_classified_as_a_depositing_aerosol_not_a_halogen() {
    let inv = CoreInventory::unit(&NUCLIDES, 2, 3);
    let out = accident_release(&inv, &heating_transient(51), &plant()).unwrap();

    let find = |label: &str| {
        out.source_term
            .nuclides
            .iter()
            .find(|n| n.label == label)
            .map(|n| n.deposition_group)
    };

    if let Some(g) = find("Te-132") {
        assert_eq!(
            g,
            DepositionGroup::Aerosol,
            "Te travels like a halogen but deposits like an aerosol"
        );
    }
    if let Some(g) = find("I-131") {
        assert_eq!(g, DepositionGroup::Halogen);
    }
    if let Some(g) = find("Kr-88") {
        assert_eq!(g, DepositionGroup::NobleGas);
    }
    if let Some(g) = find("Cs-137") {
        assert_eq!(g, DepositionGroup::Aerosol);
    }
}

/// The release is linear in the inventory, so scaling every nuclide by the
/// same factor must scale every window's release by it too. This is what makes
/// `CoreInventory::unit` useful as a shakedown: a real run differs from it by
/// a per-nuclide scale factor and nothing else.
#[test]
fn the_release_is_linear_in_the_inventory() {
    let transient = heating_transient(41);
    let one = CoreInventory::unit(&NUCLIDES, 2, 3);
    let mut ten = one.clone();
    for n in &mut ten.nuclides {
        for a in &mut n.per_ring {
            *a = units::from_curies(10.0);
        }
    }

    let a = accident_release(&one, &transient, &plant()).unwrap();
    let b = accident_release(&ten, &transient, &plant()).unwrap();

    assert_eq!(a.source_term.windows.len(), b.source_term.windows.len());
    let mut compared = 0usize;
    for (na, nb) in a.source_term.nuclides.iter().zip(&b.source_term.nuclides) {
        assert_eq!(na.label, nb.label);
        for (qa, qb) in na.released.iter().zip(&nb.released) {
            let x = qa.get::<becquerel>();
            let y = qb.get::<becquerel>();
            if x == 0.0 {
                assert_eq!(y, 0.0, "{} : zero must scale to zero", na.label);
                continue;
            }
            let rel = (y - 10.0 * x).abs() / (10.0 * x);
            assert!(
                rel < 1e-9,
                "{}: {x:e} Bq scaled by 10 gave {y:e}, rel {rel:e}",
                na.label
            );
            compared += 1;
        }
    }
    assert!(
        compared > 0,
        "the test compared nothing -- every release was zero, so it proved nothing"
    );
}

/// The half-life screen drops nuclides whose half-life is small against the
/// accident duration, and the threshold is RELATIVE — so lengthening the
/// transient screens out more. Whatever it drops must be reported, not vanish.
#[test]
fn the_half_life_screen_reports_what_it_drops() {
    // Kr-89's half-life is 189 s. Against a 1e5 s accident that is 1.9e-3,
    // well under the 0.04 threshold, so it should be screened out.
    let names = ["Kr-85", "Kr-89", "Cs-137"];
    let inv = CoreInventory::unit(&names, 2, 3);
    let out = accident_release(&inv, &heating_transient(41), &plant()).unwrap();

    let kept: Vec<&str> = out
        .source_term
        .nuclides
        .iter()
        .map(|n| n.label.as_str())
        .collect();
    let dropped = &out.screened_out;

    assert!(
        kept.contains(&"Cs-137"),
        "a 30-year half-life must survive a 1e5 s accident"
    );
    assert_eq!(
        kept.len() + dropped.len(),
        names.len(),
        "every supplied nuclide must be either kept or reported as dropped, \
         never silently lost: kept {kept:?}, dropped {dropped:?}"
    );
    if dropped.iter().any(|d| d == "Kr-89") {
        assert!(!kept.contains(&"Kr-89"));
    }
}

/// A transient that heats, cools and reheats produces a **gappy** venting mask
/// — the regime where upstream's prefix pairing is wrong. This crate must
/// notice and say so, because a result computed on a gappy mask cannot be
/// compared against upstream at all.
#[test]
fn a_heat_cool_reheat_transient_is_flagged_as_incomparable_with_upstream() {
    let times: Vec<Time> = (0..41)
        .map(|i| Time::new::<second>(i as f64 * 2500.0))
        .collect();
    let profile: Vec<f64> = (0..41)
        .map(|i| match i {
            0..=13 => 800.0 + 60.0 * i as f64,
            14..=26 => 1580.0 - 40.0 * (i - 13) as f64,
            _ => 1060.0 + 40.0 * (i - 26) as f64,
        })
        .collect();
    let temps = vec![profile.iter().map(|t| vec![c(*t); 3]).collect::<Vec<_>>(); 2];
    let transient = TemperatureTransient::from_nodes(times, temps).unwrap();

    let inv = CoreInventory::unit(&["Cs-137", "Kr-85"], 2, 3);
    let out = accident_release(&inv, &transient, &plant()).unwrap();

    assert!(
        !out.venting.is_contiguous(),
        "a heat-cool-reheat transient should produce a gappy venting mask; if it \
         does not, this test is no longer exercising the regime it was written for"
    );
    assert!(
        out.caveats.venting_mask_was_gappy,
        "a gappy mask must be reported -- it is the condition under which \
         upstream's own pairing is wrong"
    );
    // The source term is still well-formed: windows span venting sample i to
    // i+1, so a cooling period becomes one long window, never a hole.
    for pair in out.source_term.windows.windows(2) {
        assert_eq!(pair[0].end.get::<second>(), pair[1].start.get::<second>());
    }
}

/// Reports the actual numbers, so the V&V documentation rule is satisfied by
/// something a reader can re-run rather than by a claim.
///
/// # Methodology
/// `CoreInventory::unit` (1 Ci per nuclide per ring, 2 rings x 3 axial nodes),
/// NP-MHTGR reference geometry, accident-phase fractions 1e-4 / 1e-4,
/// `x_liftoff` 0. Transient: 600 C to 1600 C over 1e4 s, held to 1e5 s, 51
/// samples, uniform across all nodes. Released fraction is the total released
/// activity divided by the 6 Ci in the core for that nuclide.
///
/// # Results (measured 2026-09-21; run with `-- --nocapture` to reproduce)
/// Reported in the output. The **fraction must lie in [0, 1]** for every
/// nuclide — a release fraction above 1 would mean more came out than was
/// there, which is the coarsest possible check and the one most likely to
/// catch a units or bookkeeping error. That bound is asserted.
///
/// This is a harness check on a shakedown inventory, **not physics V&V**.
#[test]
fn the_released_fractions_are_physical_and_are_reported() {
    let inv = CoreInventory::unit(&NUCLIDES, 2, 3);
    let out = accident_release(&inv, &heating_transient(51), &plant()).unwrap();

    println!("\nreleased fraction of a 6 Ci core inventory, per nuclide");
    println!("transient: 600 C -> 1600 C over 1e4 s, held to 1e5 s");
    println!("{:>10}  {:>10}  {:>14}  {:>9}", "nuclide", "group", "released [Ci]", "fraction");

    let in_core_curies = 6.0_f64;
    for n in &out.source_term.nuclides {
        let released: f64 = n
            .released
            .iter()
            .map(|a| units::in_curies(*a))
            .sum();
        let fraction = released / in_core_curies;
        println!(
            "{:>10}  {:>10}  {released:>14.4e}  {fraction:>9.3e}",
            n.label,
            n.deposition_group.label()
        );
        assert!(
            (0.0..=1.0).contains(&fraction),
            "{}: released fraction {fraction:e} is outside [0, 1] -- more came out \
             than was in the core, which is a bookkeeping or units error",
            n.label
        );
    }

    println!("\ncaveats that fired:");
    for line in out.caveats.lines() {
        println!("  * {line}");
    }
    if !out.screened_out.is_empty() {
        println!("\nscreened out by the half-life test: {:?}", out.screened_out);
    }
    println!(
        "\nventing: {} of {} samples, contiguous = {}",
        out.venting.len(),
        out.venting.full_len(),
        out.venting.is_contiguous()
    );
}

/// **Why four of the five nuclides above release an identical fraction.**
///
/// It is not a coincidence and it is not a defect: the NP-MHTGR kernel
/// diffusion correlation is dispatched **per Z-bucket, not per nuclide**
/// (`boon-lay/src/triso_atops_fork/diffusion/mod.rs:133-169`):
///
/// | bucket | Z | elements |
/// |---|---|---|
/// | 1 | 34, 36, 52, 53, 54 | Se, Kr, Te, I, Xe |
/// | 2 | 37, 55 | Rb, Cs |
/// | 3 | 38, 56, 63 | Sr, Ba, Eu |
/// | 4 | 46, 47 | Pd, Ag |
///
/// and `release_fraction_transient` uses `z` for nothing else on the kernel
/// path except to branch silver (`z == 47`) into the breakthrough model. So
/// two nuclides in the same bucket have the same `D`, the same
/// `booth_transient` argument, and therefore a **bit-identical** release
/// fraction. Kr-85, Kr-88, I-131 and Te-132 are all in bucket 1.
///
/// Note also that the half-life drops out entirely: `bridge_node` converts an
/// activity to atoms with `A / lambda` and `atoms_to_curies` converts back with
/// `atoms * lambda`, so `lambda` cancels exactly. Kr-85 (10.7 y) and Kr-88
/// (2.8 h) releasing the same is that cancellation, and it is correct — decay
/// *in transit* is `changi`'s job, not this crate's.
///
/// # How this was established, because two earlier explanations were wrong
///
/// The first hypothesis was that the nuclide-independent as-fabricated defect
/// pathway dominated. An ablation turning those fractions down four orders of
/// magnitude dropped the release four orders — so it does dominate the
/// **magnitude** — but the two nuclides *still* agreed, so it did not explain
/// the independence. The second hypothesis was saturation: that at 1600 C for
/// 1e5 s both release fractions had reached 1. That predicted a cooler,
/// shorter transient would differentiate them; it was run and they came back
/// **exactly** equal. Both are recorded here rather than deleted, because a
/// refuted prediction is what makes the surviving explanation worth anything.
#[test]
fn nuclides_sharing_a_diffusion_bucket_release_identically() {
    let transient = heating_transient(51);
    let inv = CoreInventory::unit(&NUCLIDES, 2, 3);
    let out = accident_release(&inv, &transient, &plant()).unwrap();

    let released = |label: &str| {
        out.source_term
            .nuclides
            .iter()
            .find(|n| n.label == label)
            .map(|n| n.released.iter().map(|a| units::in_curies(*a)).sum::<f64>())
    };

    // Bucket 1: Kr (36), Te (52), I (53). Same correlation, same release.
    let bucket_one: Vec<(&str, f64)> = ["Kr-85", "Kr-88", "I-131", "Te-132"]
        .iter()
        .filter_map(|l| released(l).map(|v| (*l, v)))
        .collect();
    assert!(
        bucket_one.len() >= 2,
        "need at least two bucket-1 nuclides to compare"
    );
    let reference = bucket_one[0].1;
    assert!(reference > 0.0, "bucket 1 should release something");
    for (label, v) in &bucket_one {
        let rel = (v - reference).abs() / reference;
        assert!(
            rel < 1e-12,
            "{label} is in Z-bucket 1 and must share its diffusion correlation: \
             {v:e} against {reference:e}, rel {rel:e}"
        );
    }

    // Bucket 2: Cs (55). A different correlation AND a graphite path, because
    // caesium is not volatile -- so it must differ, and by a lot.
    if let Some(cs) = released("Cs-137") {
        let rel = (cs - reference).abs() / reference;
        assert!(
            rel > 0.1,
            "Cs-137 is in a different Z-bucket and takes the graphite path, so it \
             must differ substantially from bucket 1: {cs:e} against {reference:e}"
        );
    }
}

/// The ablation that refuted the first explanation, kept because it does
/// establish something real: the as-fabricated defect pathway carries the
/// **magnitude** of the release, even though it does not explain which
/// nuclides agree.
#[test]
fn the_defect_pathway_carries_the_magnitude_of_the_release() {
    let transient = heating_transient(51);
    let inv = CoreInventory::unit(&NUCLIDES, 2, 3);

    let released = |out: &sembawang::accident::release::AccidentRelease, label: &str| {
        out.source_term
            .nuclides
            .iter()
            .find(|n| n.label == label)
            .map(|n| n.released.iter().map(|a| units::in_curies(*a)).sum::<f64>())
            .unwrap_or(0.0)
    };

    let baseline = accident_release(&inv, &transient, &plant()).unwrap();
    let kr = released(&baseline, "Kr-85");

    let mut ablated = plant();
    ablated.fractions.heavy_metal = 1.0e-8;
    ablated.fractions.sic = 1.0e-8;
    ablated.fractions.incremental = 1.0e-9;
    ablated.fractions.incremental_sic = 1.0e-9;
    ablated.fractions.incremental_accident = 1.0e-9;
    ablated.fractions.incremental_sic_accident = 1.0e-9;
    let out = accident_release(&inv, &transient, &ablated).unwrap();
    let kr_ablated = released(&out, "Kr-85");

    println!("\nablation of the as-fabricated defect pathway (Kr-85)");
    println!("  baseline {kr:.4e} Ci -> ablated {kr_ablated:.4e} Ci");

    assert!(kr > 0.0 && kr_ablated > 0.0);
    assert!(
        kr_ablated < kr * 1.0e-3,
        "turning the defect fractions down by four orders must drop the release \
         by orders too if that pathway is what carries it: {kr:e} -> {kr_ablated:e}"
    );
}

/// The second refuted prediction, kept as evidence rather than deleted.
///
/// If the nuclide-independence were **saturation** — both release fractions
/// having reached 1 — a cooler, shorter transient would differentiate them.
/// It does not: they come back exactly equal, which is what sent the
/// investigation to the diffusion correlation table where the real answer was.
#[test]
fn a_cooler_shorter_transient_does_not_differentiate_them_either() {
    let cool = TemperatureTransient::from_ramp(
        c(700.0),
        c(900.0),
        Time::new::<second>(2.0e3),
        Time::new::<second>(1.0e4),
        41,
        2,
        3,
    )
    .unwrap();
    let inv = CoreInventory::unit(&["Kr-85", "I-131", "Cs-137"], 2, 3);
    let out = accident_release(&inv, &cool, &plant()).unwrap();

    let released = |label: &str| {
        out.source_term
            .nuclides
            .iter()
            .find(|n| n.label == label)
            .map(|n| n.released.iter().map(|a| units::in_curies(*a)).sum::<f64>())
            .unwrap_or(0.0)
    };
    let kr = released("Kr-85");
    let i = released("I-131");
    let cs = released("Cs-137");
    println!("\ncool short transient (700->900 C over 2e3 s, to 1e4 s)");
    println!("  Kr-85 {kr:.6e} Ci   I-131 {i:.6e} Ci   Cs-137 {cs:.6e} Ci");

    assert!(kr > 0.0);
    assert_eq!(
        kr, i,
        "saturation was the hypothesis; it predicted these would differ here. \
         They do not -- the cause is the shared Z-bucket diffusion correlation."
    );
    assert_ne!(cs, kr, "Cs-137 is in a different bucket and must still differ");
}
