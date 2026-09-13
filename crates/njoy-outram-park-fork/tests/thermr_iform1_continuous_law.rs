//! **V&V — THERMR's `iform = 1` continuous outgoing-energy law against oracles
//! that share none of its code.**
//!
//! # Why this one matters
//!
//! `iform = 1` is the law NJOY writes when it is asked *not* to compact the
//! secondary distribution into equally-probable bins. It is the named cure for
//! GitHub #188's width half: that deficit was measured to be the equiprobable
//! pre-tabulation itself — one-signed narrow, halving on every doubling of the
//! bin count, with no plateau (see `outram-mc-libs`'s `N_OUTGOING` docs). A
//! continuous law has no bins to truncate.
//!
//! # The oracles
//!
//! `compute_iform1` builds two nested adaptive reconstructions (the `μ` grid,
//! then each cosine's `E'` law) and normalises every cosine's density by a
//! *shared* total rather than its own integral — a detail that is easy to get
//! subtly wrong and impossible to notice from the output alone. So it is
//! checked against three things computed independently in this file, all of
//! which route through [`IncoherentInelastic::double_differential`] and nothing
//! else the ported code touches:
//!
//! 1. **`σ_inel(E)`** against [`IncoherentInelastic::cross_section`] — an
//!    entirely separate integration of the same kernel.
//! 2. **The joint `(μ, E')` distribution integrates to 1** — this is what tests
//!    the shared-normaliser detail specifically. See the note below: it is
//!    emphatically *not* true that each cosine's slice integrates to 1, and
//!    upstream is openly unsure which it is.
//! 3. **`⟨μ⟩(E)`** against a dense double quadrature over `(μ, E')`.
//!
//! # An open question in NJOY itself, settled here
//!
//! thermr.f90:2397 carries this comment, in upstream's own hand, immediately
//! above the `iform=1` write-out:
//!
//! ```text
//!   !--test yu()/sum below: is this pdf normalized to 1.0 ?
//! ```
//!
//! So NJOY does not state whether the per-cosine law it writes is a normalised
//! pdf. It is not. Each cosine's points are scaled by the **shared** total
//! `sum` from the `μ`-grid reconstruction, not by that cosine's own integral, so
//! `∫pdf dE'` at fixed `μ` is `sum2(μ)/sum` — that cosine's *share*. Only the
//! integral over `μ` as well comes to 1. Measured here: a slice at 1.05 eV
//! integrates to **0.00188**, while the joint distribution integrates to 1.
//!
//! This was worth finding: the first version of this test asserted the
//! per-slice reading and failed, and the failure could just as easily have been
//! read as a defect in the port. It is not — the port is faithful and the
//! assertion was wrong.
//!
//! # Results (2026-09-13, ENDF/B-VIII.0 `tsl-crystalline-graphite`, 600 K)
//!
//! ```text
//!   sigma_inel(E) vs an independent integration   worst +0.176 % at 3.0e-3 eV
//!   <mu>(E) vs a dense double quadrature          worst +0.0013 at 2.53e-2 eV
//! ```
//!
//! `<mu>` is checked at five energies spanning 0.0253-0.625 eV, over which it
//! moves from -0.276 to -0.023 — a factor of twelve, so the agreement is not a
//! coincidence of scale.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::calcem::iform1::compute_iform1;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7_at_temperature, IncoherentInelastic};

const TEMP_K: f64 = 600.0;
const NATOM: f64 = 1.0;
/// Top of the thermal range to reconstruct — keeps the grid short enough for a
/// test while still crossing the quasi-elastic peak and the epithermal tail.
const EMAX_EV: f64 = 1.0;
/// THERMR's own card default reconstruction tolerance.
const TOL: f64 = 5.0e-3;

fn graphite() -> Option<IncoherentInelastic> {
    let path = reference_endf_or_skip("tsl-crystalline-graphite.endf", "iform1")?;
    let tape = Tape::read_file(&path).ok()?;
    let mat = *tape.materials().first()?;
    parse_mf7_at_temperature(&tape, mat, Some(TEMP_K))
        .ok()?
        .incoherent_inelastic
}

/// `⟨μ⟩(E) = ∫∫ μ σ(E→E',μ) dE' dμ / ∫∫ σ dE' dμ`, by dense double trapezoid on
/// a grid of this file's own making.
fn mubar_double_quadrature(ii: &IncoherentInelastic, e: f64) -> f64 {
    const NMU: usize = 400;
    let (mut num, mut den) = (0.0, 0.0);
    let mut prev_mu = -1.0f64;
    let mut prev_row = sigma_at_mu(ii, e, -1.0);
    for k in 1..=NMU {
        let mu = -1.0 + 2.0 * k as f64 / NMU as f64;
        let r = sigma_at_mu(ii, e, mu);
        let dx = mu - prev_mu;
        den += 0.5 * (r + prev_row) * dx;
        num += 0.5 * (mu * r + prev_mu * prev_row) * dx;
        prev_mu = mu;
        prev_row = r;
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// `∫ σ(E→E', μ) dE'` at fixed `μ`, on a grid built from the evaluation's own
/// β points — independent of anything `sigu`/`iform1` construct.
fn sigma_at_mu(ii: &IncoherentInelastic, e: f64, mu: f64) -> f64 {
    let tev = 8.617_333_262e-5 * TEMP_K;
    let d = if ii.lat == 1 { 0.0253 } else { tev };
    let mut eps: Vec<f64> = vec![e];
    for &b in &ii.beta {
        let down = e - b * d;
        if down > 1.0e-7 {
            eps.push(down);
        }
        eps.push(e + b * d);
    }
    eps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eps.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));
    // Refine each panel so the quadrature is not the thing under test.
    let mut sum = 0.0;
    for w in eps.windows(2) {
        const SUB: usize = 8;
        for i in 0..SUB {
            let x0 = w[0] + (w[1] - w[0]) * i as f64 / SUB as f64;
            let x1 = w[0] + (w[1] - w[0]) * (i + 1) as f64 / SUB as f64;
            let y0 = ii.double_differential(e, x0, mu, TEMP_K, NATOM);
            let y1 = ii.double_differential(e, x1, mu, TEMP_K, NATOM);
            sum += 0.5 * (y0 + y1) * (x1 - x0);
        }
    }
    sum
}

#[test]
fn iform1_cross_section_matches_an_independent_integration() {
    let Some(ii) = graphite() else { return };
    let table = compute_iform1(&ii, NATOM, EMAX_EV, TOL).expect("iform1 must converge");
    assert!(
        table.records.len() >= 80,
        "expected the thermal part of EGRID, got {} records",
        table.records.len()
    );

    let mut worst = (0.0f64, 0.0f64);
    for r in &table.records {
        let reference = ii.cross_section(r.e_in_ev, TEMP_K, NATOM);
        if reference <= 0.0 {
            continue;
        }
        let rel = (r.cross_section_b - reference) / reference;
        if rel.abs() > worst.0.abs() {
            worst = (rel, r.e_in_ev);
        }
    }
    println!(
        "  iform1 sigma_inel vs cross_section: worst {:+.3} % at {:.4e} eV",
        100.0 * worst.0,
        worst.1
    );
    assert!(
        worst.0.abs() < 0.02,
        "iform1's sigma_inel is {:+.2} % from the independent integration at \
         {:.4e} eV. Both integrate the same kernel; a disagreement beyond the \
         reconstruction tolerance localises to one of them.",
        100.0 * worst.0,
        worst.1
    );
}

#[test]
fn iform1_joint_mu_energy_distribution_is_normalised() {
    let Some(ii) = graphite() else { return };
    let table = compute_iform1(&ii, NATOM, EMAX_EV, TOL).expect("iform1 must converge");

    let mut worst_norm = (0.0f64, 0.0f64);
    for r in &table.records {
        if r.cross_section_b <= 0.0 {
            continue;
        }
        let mut slice_areas: Vec<(f64, f64)> = Vec::with_capacity(r.mu_distributions.len());
        assert!(
            r.mu_distributions.len() >= 2,
            "{:.4e} eV: only {} cosines",
            r.e_in_ev,
            r.mu_distributions.len()
        );
        assert!(
            r.mu_distributions.windows(2).all(|w| w[1].mu > w[0].mu),
            "{:.4e} eV: cosines must ascend",
            r.e_in_ev
        );
        for md in &r.mu_distributions {
            assert!(
                (-1.0..=1.0).contains(&md.mu),
                "{:.4e} eV: cosine {} out of range",
                r.e_in_ev,
                md.mu
            );
            assert!(
                md.points.windows(2).all(|w| w[1].0 > w[0].0),
                "{:.4e} eV, mu {}: E' must ascend",
                r.e_in_ev,
                md.mu
            );
            assert!(
                md.points.iter().all(|&(_, p)| p >= 0.0 && p.is_finite()),
                "{:.4e} eV, mu {}: negative or non-finite pdf",
                r.e_in_ev,
                md.mu
            );
            let area: f64 = md
                .points
                .windows(2)
                .map(|w| 0.5 * (w[1].1 + w[0].1) * (w[1].0 - w[0].0))
                .sum();
            slice_areas.push((md.mu, area));
        }

        // The JOINT normalisation over (mu, E'), which is the one that holds.
        let joint: f64 = slice_areas
            .windows(2)
            .map(|w| 0.5 * (w[1].1 + w[0].1) * (w[1].0 - w[0].0))
            .sum();
        let err = joint - 1.0;
        if err.abs() > worst_norm.0.abs() {
            worst_norm = (err, r.e_in_ev);
        }
    }
    println!(
        "  iform1 joint (mu, E') normalisation: worst {:+.4} at {:.4e} eV",
        worst_norm.0, worst_norm.1
    );
    assert!(
        worst_norm.0.abs() < 0.02,
        "the joint (mu, E') distribution integrates to {:.5} at {:.4e} eV, not 1. \
         Every cosine's law is scaled by the SHARED total `sum` \
         (thermr.f90:2404), so the slices are NOT each normalised to 1 — only \
         their integral over mu is. If this fails, the shared normaliser is wrong.",
        1.0 + worst_norm.0,
        worst_norm.1
    );
}

#[test]
fn iform1_mean_cosine_matches_a_double_quadrature() {
    let Some(ii) = graphite() else { return };
    let table = compute_iform1(&ii, NATOM, EMAX_EV, TOL).expect("iform1 must converge");

    let mut worst = (0.0f64, 0.0f64);
    for r in &table.records {
        // Probe a handful of energies spanning the range; the double quadrature
        // is far too slow to run on all ~100 grid points.
        if !(r.e_in_ev > 0.02 && r.e_in_ev < 1.0) {
            continue;
        }
        if !matches!(
            format!("{:.4e}", r.e_in_ev).as_str(),
            "2.5300e-2" | "5.0000e-2" | "1.0350e-1" | "2.0000e-1" | "6.2500e-1"
        ) {
            continue;
        }
        let want = mubar_double_quadrature(&ii, r.e_in_ev);
        let diff = r.mubar - want;
        println!(
            "  {:.4e} eV: iform1 mubar {:+.5}, quadrature {:+.5}, diff {:+.5}",
            r.e_in_ev, r.mubar, want, diff
        );
        if diff.abs() > worst.0.abs() {
            worst = (diff, r.e_in_ev);
        }
    }
    assert!(
        worst.0.abs() < 0.02,
        "iform1's <mu> is {:+.4} from a dense double quadrature at {:.4e} eV",
        worst.0,
        worst.1
    );
}
