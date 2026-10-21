//! **MF=6 LAW=7 converted into the samplers' own form, checked against the raw
//! tables it came from.**
//!
//! # What landed and why it converts rather than adding a law
//!
//! LAW=7 tabulates the joint density `f(mu, E')` directly. The transport
//! samplers want the marginal `f(E')` plus the conditional `P(mu|E')`, and both
//! come out of the same table — `f(mu_j, E') = w_j * p_j(E')`, with `w_j` the
//! per-cosine weight the parser retains as of 2026-09-16 and `p_j` that cosine's
//! unit-area spectrum. Marginalising over `mu` and slicing at fixed `E'` gives
//! exactly `ChiTabular` + a per-row cosine CDF, which is what every other
//! continuum law in this crate already produces. Same reasoning as LAW=6: **no
//! new sampling path, no kernel change, no second implementation to drift.**
//!
//! The one construction step is a merged `E'` grid — the union of every cosine's
//! own knots. That is exact, not approximate: evaluating a piecewise-linear
//! density on a superset of its own knots reproduces it identically.
//!
//! # Methodology
//!
//! Be-9 MT=16 is the only LAW=7 neutron subsection in `reference-data/endf/`.
//! The test parses it **twice**: once through
//! `ContinuumEmission::from_endf_mf6` (the converted form transport consumes)
//! and once through `parse_mf6_law7_lab_angle_energy` (the raw tables). It then
//! recomputes, from the raw tables and independently of the converter:
//!
//! - `<E'>` — the joint mean outgoing energy, `int int E' f(mu, E') dE' dmu`
//! - `<mu>` — the joint mean cosine, `int int mu f(mu, E') dE' dmu`
//!
//! and compares them against the same quantities read back out of the converted
//! structures (the `ChiTabular` row's own pdf, and the per-row `mubar` weighted
//! by that pdf). Both integrals are done in **closed form on each linear
//! segment**, never by the trapezoid rule: trapezoid is exact for `int f` but
//! not for `int x f`, which is quadratic — the error that produced a false
//! "+0.60 % systematic bias" earlier in this port's history.
//!
//! Pass criterion: both means agree to 1e-9 relative. They are two arrangements
//! of the same numbers, so anything larger is a conversion defect, not
//! discretisation.
//!
//! # Results
//!
//! Printed per incident energy by the test. Recorded at introduction
//! (2026-09-16, ENDF/B-VIII.0 Be-9 MT=16): agreement at the 1e-12 level on every
//! incident energy above threshold, `<mu>` running from near zero at threshold
//! to the forward-peaked values the evaluation carries at 20 MeV.
//!
//! # What this does NOT claim
//!
//! It checks that the conversion preserves the evaluation's own distribution. It
//! is **not** a physics validation of Be-9 (n,2n), and Be-9 appears in none of
//! this workspace's criticality cases, so nothing here moves a `k`. What it
//! closes is a silent fallback: before this, Be-9 MT=16 emitted from the
//! Weisskopf stand-in with nothing recording that its evaluated law existed.

use njoy_outram_park_fork::acer::energy::parse_mf6_law7_lab_angle_energy;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::{ContinuumAngular, ContinuumEmission};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

/// `int f dx` over a piecewise-linear table — trapezoid, which is exact here.
fn integral(x: &[f64], f: &[f64]) -> f64 {
    (1..x.len().min(f.len()))
        .map(|i| 0.5 * (f[i] + f[i - 1]) * (x[i] - x[i - 1]))
        .sum()
}

/// `int x f(x) dx` over a piecewise-linear table, in **closed form**.
///
/// `x f(x)` is quadratic on each segment, so the trapezoid rule is *not* exact
/// for it. The closed form on `[x0, x1]` is
/// `h * (x0*(2f0 + f1) + x1*(f0 + 2f1)) / 6`.
fn first_moment(x: &[f64], f: &[f64]) -> f64 {
    (1..x.len().min(f.len()))
        .map(|i| {
            let (a, b) = (x[i - 1], x[i]);
            let (fa, fb) = (f[i - 1], f[i]);
            (b - a) * (a * (2.0 * fa + fb) + b * (fa + 2.0 * fb)) / 6.0
        })
        .sum()
}

#[test]
fn law7_conversion_preserves_the_evaluations_moments() {
    let Some(p) = reference_endf_or_skip("n-004_Be_009-ENDF8.0.endf", "Be-9 (MF=6 LAW=7)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("Be-9 tape parses");
    let mat = tape.materials()[0];

    // The converted form transport consumes.
    let em = ContinuumEmission::from_endf_mf6(&tape, mat, 16)
        .expect("MF=6 parses")
        .expect(
            "Be-9 MT=16 produced no continuum law. Its MF=6 is LAW=7; before 2026-09-16 this \
             fell back to the Weisskopf stand-in silently.",
        );
    assert!(
        !em.cm_frame,
        "LAW=7 is laboratory-frame by definition (ENDF-102), regardless of the section's LCT. \
         A true cm_frame here would put a lab spectrum through the CM->lab transform."
    );
    assert_eq!(em.branches.len(), 1);
    let branch = &em.branches[0];
    let ang = match &branch.angular {
        ContinuumAngular::LabTabulated(t) => t,
        other => panic!("expected ContinuumAngular::LabTabulated, got {other:?}"),
    };
    assert!(
        branch.angular.is_anisotropic(),
        "the converted LAW=7 law came back isotropic on every row. The per-cosine weights it \
         is built from have a spread of 3.67 (see mf6_law7_mu_weights.rs), so a flat result \
         means the conversion is dropping them."
    );

    // The raw tables, parsed independently.
    let sec = tape.section(mat, 6, 16).expect("Be-9 has MF=6 MT=16");
    let law7 = parse_mf6_law7_lab_angle_energy(&sec).expect("Be-9 MT=16 is ZAP=1 LAW=7");

    assert_eq!(
        branch.spectrum.incident.len(),
        law7.incident.len(),
        "converted incident grid lost or gained rows"
    );

    let mut worst_e = 0.0f64;
    let mut worst_mu = 0.0f64;
    let mut compared = 0usize;

    for (i, inc) in law7.incident.iter().enumerate() {
        // --- oracle, straight from the raw tables ---------------------------
        // norm = int int f, num_e = int int E' f, num_mu = int int mu f.
        // The mu integration is trapezoid (= ENDF INTMU 2) for `norm` and
        // `num_e`, and closed-form for `num_mu` since mu*f is quadratic in mu.
        let per_mu_norm: Vec<f64> = inc
            .tables
            .iter()
            .map(|t| t.weight * integral(&t.e_out_mev, &t.pdf))
            .collect();
        let per_mu_e: Vec<f64> = inc
            .tables
            .iter()
            .map(|t| t.weight * first_moment(&t.e_out_mev, &t.pdf))
            .collect();
        let norm = integral(&inc.mu, &per_mu_norm);
        if norm <= 0.0 {
            continue; // threshold row: every spectrum is zero
        }
        let num_e = integral(&inc.mu, &per_mu_e);
        let num_mu = first_moment(&inc.mu, &per_mu_norm);
        let (ref_e_mev, ref_mu) = (num_e / norm, num_mu / norm);

        // --- the same two numbers, read back out of the converted form ------
        let tab = &branch.spectrum.tables[i];
        let got_e_ev = first_moment(&tab.e_out, &tab.pdf) / integral(&tab.e_out, &tab.pdf);
        // <mu> = int <mu|E'> f(E') dE', with <mu|E'> the row's own exact mubar.
        let mubars: Vec<f64> = ang[i].rows.iter().map(|r| r.mubar).collect();
        let w: Vec<f64> = tab
            .pdf
            .iter()
            .zip(mubars.iter())
            .map(|(&p, &m)| p * m)
            .collect();
        let got_mu = integral(&tab.e_out, &w) / integral(&tab.e_out, &tab.pdf);

        let d_e = ((got_e_ev / 1.0e6) - ref_e_mev).abs() / ref_e_mev.abs().max(1e-30);
        let d_mu = (got_mu - ref_mu).abs() / ref_mu.abs().max(1e-3);
        worst_e = worst_e.max(d_e);
        worst_mu = worst_mu.max(d_mu);
        compared += 1;

        println!(
            "   E_in = {:8.4} MeV: <E'> {:.6e} MeV (rel diff {:.2e}), <mu> {:+.6} (rel diff {:.2e})",
            inc.e_in_mev, ref_e_mev, d_e, ref_mu, d_mu
        );
    }

    assert!(
        compared >= 20,
        "only {compared} incident energies had a non-zero law; expected ~23 above Be-9's \
         (n,2n) threshold"
    );
    println!("   worst relative difference: <E'> {worst_e:.3e}, <mu> {worst_mu:.3e}");
    assert!(
        worst_e < 1.0e-9,
        "converted <E'> differs from the raw LAW=7 tables by {worst_e:.3e} relative. These are \
         two arrangements of the same numbers, so this is a conversion defect, not \
         discretisation."
    );
    assert!(
        worst_mu < 1.0e-9,
        "converted <mu> differs from the raw LAW=7 tables by {worst_mu:.3e} relative. The \
         conditional P(mu|E') or the weighting by f(E') is wrong."
    );
}
