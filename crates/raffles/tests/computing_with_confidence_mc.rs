//! # Computing with Confidence — secondary reference: Monte Carlo (Octave)
//!
//! AI-assisted draft; see `docs/computing-with-confidence-verification.md`.
//!
//! **Methodology.** Case studies 1 and 2 have no repeated events, so under
//! independence the system c-box edges are exactly the system function of
//! independent Beta draws. `octave_inputs/indep_mc_crosscheck.m` does that
//! Monte Carlo (20 replicates of 10^5 draws, GNU Octave 11.3.0) and prints,
//! for the 95 % end points, the replicate mean and standard deviation, which
//! are read here with `include_str!`.
//!
//! The pba cut is an **outer** bound, because the cell construction rounds
//! outward at every operation (`conv.pbox`: sorted cell products, lower bound
//! from the left cell edge, upper from the right). So the right comparison is
//! enclosure, plus convergence of the over-coverage as the ladder is refined,
//! not equality.
//!
//! **Pass criteria, fixed from the method before the runs:** (1) at every
//! ladder (200, 400, 800, 1600, 3200 steps) the pba lower end is no more than
//! 4 Monte Carlo standard deviations above the Monte Carlo lower end, and the
//! upper end no more than 4 below; (2) the relative excess of the pba interval
//! over the Monte Carlo one shrinks at every doubling; (3) at 3200 steps it is
//! under half its 200-step value. The 4 standard deviations come from the
//! replicates, not from the Rust result.
//!
//! **Results:** see the V&V document (measured 2026-10-20).

use raffles::imprecise::Pbox;
use raffles::scram::pba::{and_independent, kn, or_independent, point};

const OCTAVE: &str = include_str!(
    "../verification_and_validation/computing_with_confidence/octave_inputs/indep_mc_crosscheck.octave_output.txt"
);

fn octave(key: &str) -> (f64, f64) {
    let l = OCTAVE.lines().find(|l| l.starts_with(key)).unwrap();
    let mut it = l.split_whitespace().skip(1);
    (
        it.next().unwrap().parse().unwrap(),
        it.next().unwrap().parse().unwrap(),
    )
}

fn case1_top(levels: usize) -> (f64, f64) {
    let c: Vec<Pbox> = [
        (0, 2000),
        (3, 500),
        (1, 150),
        (0, 460),
        (7, 10000),
        (0, 380),
    ]
    .iter()
    .map(|(k, n)| kn(*k, *n, levels).unwrap())
    .collect();
    let e5 = or_independent(&c[4], &c[5]).unwrap();
    let e4 = or_independent(&e5, &c[3]).unwrap();
    let e3 = and_independent(&e4, &c[2]).unwrap();
    let e2 = or_independent(&e3, &c[1]).unwrap();
    let cut = or_independent(&e2, &c[0]).unwrap().cut(0.05).unwrap();
    (cut.lower(), cut.upper())
}

fn case2_top(levels: usize) -> (f64, f64) {
    let d: Vec<Pbox> = [(1, 42), (8, 42), (8, 42)]
        .iter()
        .map(|(k, n)| kn(*k, *n, levels).unwrap())
        .collect();
    let ov = point(3.49e-5, levels).unwrap();
    let a = or_independent(&d[1], &ov).unwrap();
    let b = or_independent(&d[2], &ov).unwrap();
    let cut = and_independent(&d[0], &or_independent(&a, &b).unwrap())
        .unwrap()
        .cut(0.05)
        .unwrap();
    (cut.lower(), cut.upper())
}

#[test]
fn independence_is_an_outer_bound_of_monte_carlo_and_converges() {
    for (name, f, lo_key, hi_key) in [
        (
            "case1",
            case1_top as fn(usize) -> (f64, f64),
            "case1_e1i_lower",
            "case1_e1i_upper",
        ),
        (
            "case2",
            case2_top as fn(usize) -> (f64, f64),
            "case2_e1i_lower",
            "case2_e1i_upper",
        ),
    ] {
        let (lo_mc, lo_sd) = octave(lo_key);
        let (hi_mc, hi_sd) = octave(hi_key);
        let mut prev_excess = f64::INFINITY;
        let mut first_excess = 0.0;
        let mut last_excess = 0.0;
        for levels in [201, 401, 801, 1601, 3201] {
            let (lo, hi) = f(levels);
            println!(
                "{name} levels {levels}: pba [{lo:.6e}, {hi:.6e}] MC [{lo_mc:.6e}, {hi_mc:.6e}] (sd {lo_sd:.2e}, {hi_sd:.2e})"
            );
            assert!(
                lo <= lo_mc + 4.0 * lo_sd,
                "{name} {levels}: lower end above MC"
            );
            assert!(
                hi >= hi_mc - 4.0 * hi_sd,
                "{name} {levels}: upper end below MC"
            );
            let excess = (hi - hi_mc) / hi_mc + (lo_mc - lo) / lo_mc;
            assert!(
                excess < prev_excess,
                "{name}: excess did not shrink at {levels}"
            );
            if levels == 201 {
                first_excess = excess;
            }
            prev_excess = excess;
            last_excess = excess;
        }
        assert!(
            last_excess < 0.5 * first_excess,
            "{name}: {last_excess} vs {first_excess}"
        );
    }
}
