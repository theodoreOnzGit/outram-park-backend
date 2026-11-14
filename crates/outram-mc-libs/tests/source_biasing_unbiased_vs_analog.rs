// SPDX-License-Identifier: AGPL-3.0-only
//! **Source biasing is unbiased in the mean: analog against biased** (gh:#497).
//!
//! # What is under test
//!
//! OpenMC's source `bias` (distribution.cpp, ported in
//! `src/source/distribution.rs`, bit-compared with compiled OpenMC in
//! `source_distributions_vs_openmc_code_to_code.rs`): a distribution `p` is
//! SAMPLED from a bias distribution `q`, and each draw carries the importance
//! weight `w = p/q`. It is a variance-reduction choice, not physics, and is
//! opt-in (`Distribution::with_bias`); an unbiased source has `w = 1` exactly.
//!
//! # Methodology (fixed before the run)
//!
//! Energy `p = Uniform(1 MeV, 4 MeV)`. Biased arm: the same `p` with bias
//! `q = PowerLaw(1 MeV, 4 MeV, n = 2)` (density `∝ E^2`, so it
//! oversamples the high end). A first run used `n = 1`, for which `w ∝ 1/E`
//! makes `w·E` constant: a zero-variance estimator of `E[E]` (seed spread
//! exactly 0, `z` undefined), a true property of that bias but useless as a
//! test. A second run used `n = 0.5` and found `E[w] = 1.4997`, i.e. a
//! **biased** estimator: see `non_integer_power_law_bias_weight_is_upstreams_defect`.
//! `n = 2` was then fixed, before any `n = 2` result. Drawn through the full `IndependentSource::sample`
//! path (`wgt = strength * w`). Analytic truth under `p`:
//! `E[E] = 2.5e6`, `E[E^2] = 7.0e12`, `P(E < 2 MeV) = 1/3`, `E[w] = 1`.
//! Estimators are `mean(w * f(E))`. N = 5 seeds per arm, 100 000 draws per
//! seed; the estimate is the seed mean and sigma the seed-to-seed standard
//! error. `z = (biased - analog)/sqrt(s_a^2 + s_b^2)` is REPORTED AGAINST BOTH
//! bands (`|z| <= 1` and `|z| <= 2`; the maintainer has not chosen one). The
//! only assertion is a gross-defect guard `|z| <= 5` plus each arm within 5
//! sigma of the analytic truth. Not a band.
//!
//! # Results
//!
//! See the `println!` table (run with `--nocapture`) and the record in
//! `verification_and_validation/stats_epic_493/497_learned_importance.md`.

use outram_mc_libs::prelude::{
    AngleKind, Distribution, EnergyKind, IndependentSource, SpatialKind,
};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::source::angle::MonodirectionalAngle;
use outram_mc_libs::source::spatial::PointSource;

const E_LO: f64 = 1.0e6;
const E_HI: f64 = 4.0e6;
const N_SEEDS: usize = 5;
const N_DRAWS: usize = 100_000;

fn source(biased: bool) -> IndependentSource {
    source_n(biased, 2.0)
}

fn source_n(biased: bool, n: f64) -> IndependentSource {
    let p = Distribution::uniform(E_LO, E_HI);
    let d = if biased {
        p.with_bias(Distribution::power_law(E_LO, E_HI, n)).unwrap()
    } else {
        p
    };
    IndependentSource {
        spatial: SpatialKind::Point(PointSource { r: Position::ZERO }),
        energy: EnergyKind::Distribution(d),
        angle: AngleKind::Monodirectional(MonodirectionalAngle { d: Direction::new(0.0, 0.0, 1.0) }),
        strength: 1.0,
    }
}

/// Per-seed estimates of `[E[w], E[wE], E[wE^2], P(E<2MeV)]`.
fn arm(biased: bool, seed0: u64) -> Vec<[f64; 4]> {
    let src = source(biased);
    (0..N_SEEDS)
        .map(|s| {
            let mut seed = seed0 + 7_919 * s as u64;
            let mut acc = [0.0f64; 4];
            for _ in 0..N_DRAWS {
                let site = src.sample(&mut seed);
                acc[0] += site.wgt;
                acc[1] += site.wgt * site.e;
                acc[2] += site.wgt * site.e * site.e;
                if site.e < 2.0e6 {
                    acc[3] += site.wgt;
                }
            }
            acc.map(|x| x / N_DRAWS as f64)
        })
        .collect()
}

fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

#[test]
fn biased_source_mean_matches_analog_and_truth() {
    let analog = arm(false, 11);
    let biased = arm(true, 22);
    let truth = [1.0, 2.5e6, 7.0e12, 1.0 / 3.0];
    let names = ["E[w]", "E[wE]", "E[wE^2]", "P(E<2MeV)"];
    for k in 0..4 {
        let col = |a: &[[f64; 4]]| a.iter().map(|r| r[k]).collect::<Vec<f64>>();
        let (ma, sa) = mean_sem(&col(&analog));
        let (mb, sb) = mean_sem(&col(&biased));
        let z = if sa == 0.0 && sb == 0.0 { 0.0 } else { (mb - ma) / (sa * sa + sb * sb).sqrt() };
        // The analog weight is exactly 1 in every draw, so E[w] has zero seed spread:
        // then the estimate must equal the truth exactly (z defined as 0), not 0/0.
        let zt = |m: f64, s: f64| if s == 0.0 { if (m - truth[k]).abs() <= 1e-9 * truth[k].abs() { 0.0 } else { f64::INFINITY } } else { (m - truth[k]) / s };
        let za = zt(ma, sa);
        let zb = zt(mb, sb);
        println!(
            "{:<10} analog {ma:.6e} +- {sa:.2e} | biased {mb:.6e} +- {sb:.2e} | z(b-a) {z:+.2} \
             [|z|<=1: {}, |z|<=2: {}] | z_analog_vs_truth {za:+.2}, z_biased_vs_truth {zb:+.2} \
             | sigma ratio biased/analog {:.3}",
            names[k], z.abs() <= 1.0, z.abs() <= 2.0, sb / sa
        );
        assert!(z.abs() <= 5.0, "{}: gross bias guard, z = {z}", names[k]);
        assert!(za.abs() <= 5.0 && zb.abs() <= 5.0, "{}: off the analytic truth", names[k]);
    }
    // The weights must be non-trivial in the biased arm and exactly 1 in analog.
    let mut s = 5u64;
    assert_eq!(source(false).sample(&mut s).wgt, 1.0);
    let mut s = 5u64;
    assert_ne!(source(true).sample(&mut s).wgt, 1.0);
}

/// **Upstream defect, reproduced and pinned (not fixed).** OpenMC's
/// `PowerLaw::evaluate` computes `int pwr = n() + 1;` (the port notes it in
/// `Distribution::evaluate`), which truncates a non-integer exponent, so the
/// density it returns is normalised with `trunc(n + 1)` instead of `n + 1`.
/// The importance weight `p/q` uses that density, so a `PowerLaw` bias with
/// non-integer `n` gives `E[w] = (n + 1)/trunc(n + 1)` instead of 1: for
/// `n = 0.5`, exactly 1.5, a 50 % bias in every biased tally. The sampler is
/// exact; only the weight is wrong. The port copies upstream faithfully
/// (bit-compared at integer `n` only), so it inherits this. Predicted before
/// the run that found it (by reading `evaluate`): `E[w] = 1.5`.
#[test]
fn non_integer_power_law_bias_weight_is_upstreams_defect() {
    let src = source_n(true, 0.5);
    let mut seed = 99u64;
    let n = 200_000;
    let mean_w: f64 = (0..n).map(|_| src.sample(&mut seed).wgt).sum::<f64>() / n as f64;
    println!("PowerLaw bias n = 0.5: E[w] = {mean_w:.5} (unbiased would be 1, upstream defect predicts 1.5)");
    assert!((mean_w - 1.5).abs() < 0.01, "E[w] = {mean_w}");
}
