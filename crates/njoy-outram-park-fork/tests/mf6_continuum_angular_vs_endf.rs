//! The MF=6 LAW=1 **angular** half of the continuum emission law, read from the
//! evaluation rather than assumed away (bead `op-og56`).
//!
//! # Why this test exists
//!
//! `parse_law1_neutron_body` used to read ENDF `NA` only to compute the row
//! stride, take `f₀`, and discard `f₁ … f_NA`; `LANG` was never read off the
//! TAB2 at all. The consequence was that **every** MT=91 continuum neutron and
//! every MT=16 (n,2n) neutron left the collision isotropically in the frame the
//! law names, on evaluations that say otherwise.
//!
//! That gap was invisible from the port's own data structures. Once the
//! coefficients are dropped at parse time, "this evaluation declares the
//! emission isotropic" and "this port never read the angular data" produce
//! byte-identical results, and no test written against the parsed form can tell
//! them apart. This file exists to make the difference assertable, and it is
//! the control every ablation of the continuum angular law depends on: a
//! control that switches off an angular law which was flat to begin with
//! reports "no difference" and reads as "this physics does not matter".
//!
//! # Methodology
//!
//! Fixtures are the ENDF/B-VIII.0 tapes in `reference-data/endf/` (each case
//! skips when its tape is absent). For each nuclide the MF=6/MT=91 section is
//! parsed with [`parse_mf6_law1_neutrons`] and the retained angular tables are
//! judged against the tape's own declarations:
//!
//! - `LANG` must decode to the representation the tape carries.
//! - A nuclide the evaluation makes anisotropic must report
//!   [`Mf6Neutron::is_angular_isotropic`] `== false`, and one it makes isotropic
//!   must report `true` — both directions, so neither a reader that invents
//!   anisotropy nor one that flattens it can pass.
//! - Every `⟨μ⟩ = a₁ = f₁/f₀` must satisfy `|a₁| ≤ 1/3`. This is a physics
//!   invariant, not a fit: for a row with `NA = 1` the conditional density is
//!   `f(μ) = (1 + 3a₁μ)/2`, which goes negative somewhere on `[−1, 1]` as soon
//!   as `|3a₁| > 1`. ENDF requires the density be non-negative, so a violation
//!   means the coefficients have been mis-strided or mis-normalised — the two
//!   failure modes a row-major reader actually has.
//! - The angular tables must be aligned one-for-one with the energy tables,
//!   since MF=6 LAW=1 is a *correlated* law and row `i` of one describes row `i`
//!   of the other.
//!
//! # Results (measured 2026-09-16, ENDF/B-VIII.0, `cargo test --release`)
//!
//! Printed by `mf6_mt91_angular_structure_is_reported`, which re-derives the
//! whole table on every run:
//!
//! | nuclide | MT | LANG | NE | rows | rows with `NA>0` | peak `|⟨μ⟩|` |
//! |---|---|---|---|---|---|---|
//! | U-238 | 16 | Legendre | 37 | 2068 | **2066** | 0.4275 |
//! | U-238 | 91 | Legendre | 96 | 8654 | **8652** | **0.5573** |
//! | U-235 | 16 | Legendre | 39 | 2461 | **2459** | 0.4643 |
//! | U-235 | 91 | Legendre | 64 | 5296 | **5294** | **0.5751** |
//! | F-19 | 16 | Legendre | 38 | 1005 | 504 | 0.2574 |
//! | F-19 | 91 | Legendre | 13 | 175 | **0** | 0.0000 |
//! | Si-28 | 16 | Legendre | 3 | 30 | 28 | 0.0586 |
//! | Si-28 | 91 | Legendre | 11 | 205 | 205 | 0.2775 |
//! | O-16 | 16 | Kalbach-Mann | 17 | 756 | 756 | n/a |
//! | O-16 | 91 | Kalbach-Mann | 31 | 1751 | 1751 | n/a |
//! | Al-27 | 16 | Kalbach-Mann | 7 | 197 | 197 | n/a |
//! | Al-27 | 91 | Kalbach-Mann | 17 | 642 | 642 | n/a |
//!
//! **Interpretation.** On Godiva's two dominant nuclides essentially *every*
//! row of the continuum law is anisotropic — 8652 of 8654 for U-238's MT=91 —
//! with conditional mean cosines reaching `⟨μ⟩ ≈ 0.56`. That is the magnitude
//! of what sampling the continuum isotropically was discarding, and it is the
//! same order as the discrete-level anisotropy whose omission was worth
//! −198 pcm on Godiva (bead `op-tm9f`). It does not follow that the continuum
//! is worth a comparable amount: MT=91 carries a different share of collisions
//! and a different `Σ` weighting, so the worth must be **ablated, not inferred**
//! from this table.
//!
//! F-19's MT=91 is the negative control — `NA = 0` on every incident energy, so
//! the only correct answer is isotropic. Note its MT=16 is *not*: half its rows
//! carry angular structure, which is why the control is scoped to one channel
//! rather than to the nuclide.
//!
//! **O-16 and Al-27 use `LANG = 2` (Kalbach-Mann), not Legendre.** Their
//! `peak |⟨μ⟩|` reads `n/a` because [`Mf6Neutron::peak_legendre_mubar`]
//! deliberately returns `0.0` for a non-Legendre law rather than
//! mis-interpreting `r`/`a` as Legendre coefficients — the table above is a
//! *parse-level* view and that column only means anything for Legendre.
//!
//! Both representations are now sampled. Kalbach-Mann is covered by
//! `o16_mt91_is_kalbach_mann_and_reaches_transport` below, which works at the
//! built `ContinuumEmission` level rather than the parse level, because the
//! slope `a` is computed there and not read off the tape. Recording the split
//! is still the point — it is what stops a later reader assuming one code path
//! covers every evaluation.
//!
//! **This is verification of the reader, not validation of the physics.** It
//! establishes that the coefficients reaching a transport code are the ones on
//! the tape. What they are worth in `k` is a separate, ablated measurement.

use njoy_outram_park_fork::acer::energy::{parse_mf6_law1_neutrons, Mf6AngularLaw, Mf6Neutron};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

/// Load one nuclide's MF=6/MT=`mt` neutron subsections, or `None` when the tape
/// is absent (a crates.io consumer has no repository around the crate) or the
/// section does not exist.
fn mf6_neutrons(file: &str, label: &str, mat: i32, mt: i32) -> Option<Vec<Mf6Neutron>> {
    let path = reference_endf_or_skip(file, label)?;
    let tape = Tape::read_file(&path).expect("tape parses");
    let section = tape.section(mat, 6, mt)?;
    Some(parse_mf6_law1_neutrons(section).expect("MF=6 LAW=1 neutron subsections parse"))
}

/// Largest `|a₁|` over every row of every incident energy of every subsection.
fn peak_mubar(neutrons: &[Mf6Neutron]) -> f64 {
    neutrons
        .iter()
        .map(|n| n.peak_legendre_mubar())
        .fold(0.0, f64::max)
}

/// Count `(rows, rows with NA>0)` across every subsection.
fn row_counts(neutrons: &[Mf6Neutron]) -> (usize, usize) {
    let mut total = 0;
    let mut aniso = 0;
    for n in neutrons {
        for t in &n.angular {
            total += t.len();
            if t.na > 0 {
                aniso += t.len();
            }
        }
    }
    (total, aniso)
}

/// How many rows carry each `NA`, as a sorted `(na, rows)` list.
///
/// This is what decides the *sampling* strategy downstream, so it is measured
/// rather than assumed: `NA = 1` admits a closed-form inversion of the linear
/// density `(1 + 3a₁μ)/2`, while `NA ≥ 2` needs the Legendre series linearised
/// before it can be sampled at all. Which of those dominates is a property of
/// the evaluations, not a design choice.
fn na_histogram(neutrons: &[Mf6Neutron]) -> Vec<(u32, usize)> {
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for n in neutrons {
        for t in &n.angular {
            *counts.entry(t.na).or_default() += t.len();
        }
    }
    counts.into_iter().collect()
}

/// The angular tables must be aligned one-for-one with the energy tables.
///
/// MF=6 LAW=1 is a **correlated** energy-angle law: row `i` of the angular table
/// is the emission angle *conditional on* outgoing energy `i`. A reader that
/// lets the two drift out of step samples a physical angular law at the wrong
/// energy, which is worse than isotropy because it looks like real physics.
fn assert_aligned(neutrons: &[Mf6Neutron], who: &str) {
    for (k, n) in neutrons.iter().enumerate() {
        assert_eq!(
            n.angular.len(),
            n.law4.incident.len(),
            "{who} subsection {k}: {} angular tables against {} energy tables — MF=6 LAW=1 is a \
             correlated law and the two must be parallel",
            n.angular.len(),
            n.law4.incident.len(),
        );
        for (i, (a, e)) in n.angular.iter().zip(&n.law4.incident).enumerate() {
            assert_eq!(
                a.len(),
                e.e_out_mev.len(),
                "{who} subsection {k} incident table {i}: {} angular rows against {} \
                 outgoing-energy points",
                a.len(),
                e.e_out_mev.len(),
            );
            assert!(
                (a.e_in_mev - e.e_in_mev).abs() <= 1.0e-12 * e.e_in_mev.abs().max(1.0),
                "{who} subsection {k} table {i}: angular table is at {} MeV, energy table at {} MeV",
                a.e_in_mev,
                e.e_in_mev,
            );
        }
    }
}

/// Every `⟨μ⟩` must leave the conditional density non-negative.
///
/// For `NA = 1`, `f(μ) = (1 + 3a₁μ)/2`, so `|a₁| > 1/3` puts the density below
/// zero somewhere on `[−1, 1]`. ENDF does not permit that, so a violation is a
/// reader defect — a mis-strided row or a missing `f₀` normalisation — and not
/// a property of the evaluation. Rows with `NA ≥ 2` can legitimately exceed
/// `1/3` in `a₁` alone because the higher terms restore positivity, so the
/// bound is asserted only where it is a theorem.
fn assert_mubar_physical(neutrons: &[Mf6Neutron], who: &str) {
    for (k, n) in neutrons.iter().enumerate() {
        if n.lang != Mf6AngularLaw::Legendre {
            continue;
        }
        for (i, t) in n.angular.iter().enumerate() {
            if t.na != 1 {
                continue;
            }
            for r in 0..t.len() {
                let a1 = t.legendre_mubar(r);
                assert!(
                    a1.abs() <= 1.0 / 3.0 + 1.0e-9,
                    "{who} subsection {k} table {i} row {r}: a1 = {a1}, which makes the NA=1 \
                     Legendre density (1 + 3·a1·mu)/2 negative somewhere on [-1, 1]. That is a \
                     reader defect (row stride or f0 normalisation), not an evaluation property.",
                );
                assert!(
                    a1.is_finite(),
                    "{who} subsection {k} table {i} row {r}: a1 is not finite",
                );
            }
        }
    }
}

/// U-238's MT=91 continuum carries a real, correlated angular law — the data
/// this port used to discard.
///
/// This is the *positive* control: if it ever reports isotropic, either the
/// coefficients stopped being read or the fixture changed, and every ablation of
/// the continuum angular law downstream becomes meaningless.
#[test]
fn u238_mt91_continuum_is_anisotropic_on_the_tape() {
    let Some(neutrons) = mf6_neutrons("n-092_U_238.endf", "mf6-angular-u238", 9237, 91) else {
        return;
    };
    assert!(!neutrons.is_empty(), "U-238 MT=91 has a neutron subsection");
    for n in &neutrons {
        assert_eq!(
            n.lang,
            Mf6AngularLaw::Legendre,
            "U-238 MF=6/MT=91 declares LANG=1 (Legendre); got {:?}",
            n.lang,
        );
        assert_eq!(n.lct, 2, "U-238 MF=6/MT=91 is tabulated in the CM frame");
    }
    assert_aligned(&neutrons, "U-238 MT=91");
    assert_mubar_physical(&neutrons, "U-238 MT=91");

    let (total, aniso) = row_counts(&neutrons);
    let peak = peak_mubar(&neutrons);
    println!("U-238 MT=91: {aniso} of {total} rows anisotropic, peak |mu_bar| = {peak:.6}");

    assert!(
        !neutrons.iter().all(Mf6Neutron::is_angular_isotropic),
        "U-238 MT=91 must be anisotropic on the tape",
    );
    assert!(
        aniso > 0,
        "U-238 MT=91: no row carries NA>0 — the angular half is not reaching the parser",
    );
    // A floor, not the measured value: this guards "the coefficients stopped
    // being read", which collapses `peak` to exactly 0, without pinning a number
    // that a library revision may legitimately move.
    assert!(
        peak > 0.01,
        "U-238 MT=91 peak |mu_bar| = {peak}, indistinguishable from isotropic",
    );
}

/// U-235's MT=91 continuum, same claim as U-238's.
///
/// Both of Godiva's dominant nuclides are checked because the fast-spectrum
/// question this data bears on is a *sum* over them, and a reader defect that
/// happened to affect only one would otherwise pass.
#[test]
fn u235_mt91_continuum_is_anisotropic_on_the_tape() {
    let Some(neutrons) = mf6_neutrons("n-092_U_235-ENDF8.0.endf", "mf6-angular-u235", 9228, 91)
    else {
        return;
    };
    assert!(!neutrons.is_empty(), "U-235 MT=91 has a neutron subsection");
    for n in &neutrons {
        assert_eq!(n.lang, Mf6AngularLaw::Legendre);
        assert_eq!(n.lct, 2);
    }
    assert_aligned(&neutrons, "U-235 MT=91");
    assert_mubar_physical(&neutrons, "U-235 MT=91");

    let (total, aniso) = row_counts(&neutrons);
    let peak = peak_mubar(&neutrons);
    println!("U-235 MT=91: {aniso} of {total} rows anisotropic, peak |mu_bar| = {peak:.6}");

    assert!(aniso > 0, "U-235 MT=91 must carry NA>0 rows");
    assert!(peak > 0.01, "U-235 MT=91 peak |mu_bar| = {peak}");
}

/// F-19's MT=91 is genuinely isotropic — the **negative** control.
///
/// Without this, a reader that manufactured anisotropy (a stride error reading
/// the next row's `E'` as an angular coefficient, say) would pass every
/// assertion above. F-19 declares `NA = 0` on every one of its incident
/// energies, so the only correct answer here is "isotropic", and a non-zero
/// `⟨μ⟩` is a defect.
///
/// F-19 is also the nuclide whose inelastic channel carried the FHR pebble's
/// entire +4004 pcm residual (GitHub #193), so its continuum law is worth
/// pinning for its own sake.
#[test]
fn f19_mt91_continuum_is_isotropic_on_the_tape() {
    let Some(neutrons) = mf6_neutrons("n-009_F_019-ENDF8.0.endf", "mf6-angular-f19", 925, 91)
    else {
        return;
    };
    assert!(!neutrons.is_empty(), "F-19 MT=91 has a neutron subsection");
    assert_aligned(&neutrons, "F-19 MT=91");
    assert_mubar_physical(&neutrons, "F-19 MT=91");

    let (total, aniso) = row_counts(&neutrons);
    let peak = peak_mubar(&neutrons);
    println!("F-19 MT=91: {aniso} of {total} rows anisotropic, peak |mu_bar| = {peak:.6}");

    assert!(
        neutrons.iter().all(Mf6Neutron::is_angular_isotropic),
        "F-19 MT=91 declares NA=0 on every incident energy — reporting it anisotropic means the \
         reader is manufacturing angular structure, most likely a row-stride error",
    );
    assert_eq!(
        aniso, 0,
        "F-19 MT=91: {aniso} of {total} rows reported NA>0 where the tape has none",
    );
    assert_eq!(peak, 0.0, "F-19 MT=91 peak |mu_bar| must be exactly zero");
}

/// Report the angular structure of every continuum channel this workspace's
/// fixtures carry, so the size of what was being discarded is on the record
/// rather than inferred.
///
/// Deliberately assertion-light: its job is to print the table that the module
/// doc cites, and to fail only if *nothing* in the fixture set is anisotropic —
/// which would mean the reader had regressed globally rather than on one
/// nuclide.
#[test]
fn mf6_mt91_angular_structure_is_reported() {
    let cases: &[(&str, &str, i32)] = &[
        ("n-092_U_238.endf", "u238", 9237),
        ("n-092_U_235-ENDF8.0.endf", "u235", 9228),
        ("n-092_U_234-ENDF8.0.endf", "u234", 9222),
        ("n-009_F_019-ENDF8.0.endf", "f19", 925),
        ("n-006_C_012-ENDF8.0.endf", "c12", 625),
        ("n-008_O_016-ENDF8.0.endf", "o16", 825),
        ("n-013_Al_027-ENDF8.0.endf", "al27", 1325),
        ("n-014_Si_028-ENDF8.0.endf", "si28", 1425),
    ];

    println!(
        "{:<8} {:>4} {:>12} {:>5} {:>8} {:>8} {:>12}",
        "nuclide", "MT", "LANG", "NE", "rows", "aniso", "peak |mu|"
    );
    let mut any_anisotropic = false;
    let mut examined = 0;
    for &(file, label, mat) in cases {
        for mt in [16, 91] {
            let Some(neutrons) = mf6_neutrons(file, &format!("mf6-angular-{label}"), mat, mt)
            else {
                continue;
            };
            if neutrons.is_empty() {
                continue;
            }
            examined += 1;
            assert_aligned(&neutrons, &format!("{label} MT={mt}"));
            assert_mubar_physical(&neutrons, &format!("{label} MT={mt}"));

            let (total, aniso) = row_counts(&neutrons);
            let peak = peak_mubar(&neutrons);
            let ne: usize = neutrons.iter().map(|n| n.angular.len()).sum();
            any_anisotropic |= aniso > 0;
            let hist: Vec<String> = na_histogram(&neutrons)
                .iter()
                .map(|(na, rows)| format!("{na}:{rows}"))
                .collect();
            println!(
                "{:<8} {:>4} {:>12} {:>5} {:>8} {:>8} {:>12.6}  NA[{}]",
                label,
                mt,
                format!("{:?}", neutrons[0].lang),
                ne,
                total,
                aniso,
                peak,
                hist.join(" "),
            );
        }
    }

    if examined == 0 {
        return; // no fixtures present; the per-nuclide tests already skipped
    }
    assert!(
        any_anisotropic,
        "not one continuum channel in {examined} examined sections reported angular structure — \
         the MF=6 angular half has regressed to unread",
    );
}

/// O-16's MT=91 is **Kalbach-Mann**, and it now reaches transport as a sampled
/// law rather than as an unported one.
///
/// # Why this needs its own test
///
/// `LANG = 2` is a different representation, not a different parameterisation of
/// the same one: the row carries `(r)` or `(r, a)` rather than Legendre
/// coefficients, and where the evaluation stores only `r` — which O-16 and
/// Al-27 both do — the slope `a` has to come from the **Kalbach-86 systematics**,
/// a function of the projectile/ejectile/target masses. So every assertion the
/// Legendre tests make is about machinery this path does not use.
///
/// The slope is supplied by [`njoy_outram_park_fork::groupr::kinematics::bach`],
/// which was already in this crate as a port of NJOY2016 `groupr.f90:8812-8932`
/// for the GROUPR path. It is reused rather than reimplemented; a second copy of
/// the same systematics would drift from the first.
///
/// # What is asserted
///
/// 1. O-16's MT=91 builds as [`ContinuumAngular::KalbachMann`] — not
///    `Unported` (which is what it was before this landed) and not
///    `EvaluatedIsotropic`.
/// 2. The systematics produce a **forward-peaked** law whose slope **grows with
///    incident energy**. A slope that came back flat, or fell with energy, would
///    mean `bach` is being called with the wrong arguments — the likeliest error
///    being eV/MeV confusion, since `bach` takes eV and the tables are stored in
///    MeV.
/// 3. Every `⟨μ⟩` is physical and within `[0, 1)`: Kalbach emission is forward
///    or isotropic, never backward, because `r ≥ 0` and the Langevin function is
///    positive.
///
/// # Results (2026-09-16, ENDF/B-VIII.0 O-16, MAT 825)
///
/// Printed on every run. The slope rises monotonically with incident energy,
/// which is the signature of the systematics being evaluated at the right
/// energies.
#[test]
fn o16_mt91_is_kalbach_mann_and_reaches_transport() {
    use njoy_outram_park_fork::nuclear_data::secondary::{ContinuumAngular, ContinuumEmission};

    let Some(path) = reference_endf_or_skip("n-008_O_016-ENDF8.0.endf", "mf6-kalbach-o16") else {
        return;
    };
    let tape = Tape::read_file(&path).expect("tape parses");
    let Some(law) = ContinuumEmission::from_endf_mf6(&tape, 825, 91).expect("MF=6 parses") else {
        panic!("O-16 MT=91 produced no ContinuumEmission at all");
    };

    for (b, branch) in law.branches.iter().enumerate() {
        match &branch.angular {
            ContinuumAngular::KalbachMann(_) => {}
            other => panic!(
                "O-16 MT=91 branch {b} reached transport as {other:?}. The tape declares \
                 LANG = 2 with NA = 1 on all 1751 rows, so anything else means the \
                 Kalbach-Mann path is not being taken -- `Unported` in particular means it \
                 regressed to emitting isotropically."
            ),
        }
        assert!(
            branch.angular.is_anisotropic(),
            "O-16 MT=91 branch {b}: every Kalbach slope collapsed to zero. `bach` is either \
             failing or being fed the wrong energies (it takes eV; the tables are MeV)."
        );
    }

    // The slope must grow with incident energy — the systematics' defining
    // behaviour, and what an eV/MeV mix-up would destroy.
    let branch = &law.branches[0];
    let incident = &branch.spectrum.incident;
    println!("O-16 MT=91 Kalbach-Mann, pdf-weighted <mu_cm> against incident energy:");
    let mut profile: Vec<(f64, f64)> = Vec::new();
    for (i, &e_in) in incident.iter().enumerate() {
        let ce = &branch.spectrum.tables[i];
        let (mut num, mut den) = (0.0, 0.0);
        for k in 0..ce.e_out.len() {
            let Some(mu) = branch.angular.mubar(i, k) else {
                continue;
            };
            let w = ce.pdf[k];
            num += w * mu;
            den += w;
            assert!(
                (0.0..1.0).contains(&mu) && mu.is_finite(),
                "O-16 MT=91 table {i} row {k}: <mu> = {mu}, outside [0, 1). Kalbach emission \
                 is forward or isotropic, never backward -- r >= 0 and the Langevin function \
                 is positive, so a negative value is an algebra or sign error."
            );
        }
        if den > 0.0 {
            profile.push((e_in, num / den));
        }
    }
    assert!(profile.len() >= 4, "too few usable tables to judge a trend");
    for (e, mu) in profile.iter().step_by(profile.len().div_ceil(6)) {
        println!("  E_in = {e:>10.4e} eV   <mu_cm> = {mu:+.6}");
    }

    let (e_lo, mu_lo) = profile[0];
    let (e_hi, mu_hi) = profile[profile.len() - 1];
    assert!(
        mu_hi > mu_lo,
        "O-16 MT=91's mean cosine does not rise with incident energy ({mu_lo:+.6} at \
         {e_lo:.3e} eV, {mu_hi:+.6} at {e_hi:.3e} eV). The Kalbach-86 slope grows with \
         energy by construction, so this means `bach` is being evaluated at the wrong \
         energies -- check the eV/MeV conversion first."
    );
    assert!(
        mu_hi > 0.02,
        "O-16 MT=91's mean cosine only reaches {mu_hi:+.6} at {e_hi:.3e} eV, which is \
         indistinguishable from isotropic. The law would then be worth nothing and the \
         ablation that prices it would report a meaningless null."
    );
}
