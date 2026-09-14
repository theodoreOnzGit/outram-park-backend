//! Unit tests for the `PURR` port (moved verbatim out of `purr/mod.rs`
//! when the file was split at the ladder-generation / `unrest` boundary).

use super::ladder::CHISQ;
use super::unrest::line_shape;
use super::wfun::DopplerTable;
use super::*;

/// `rann` output for `idum = -101` (the seed `purr.f90`'s driver uses),
/// from a verbatim-`rann` Fortran oracle (`purr.f90:2877-2917` compiled
/// unchanged with gfortran 13.3.0, 2026-09-10): draws 1–20, then the
/// 100th, 1000th and 10000th.
const RANN_ORACLE_FIRST_20: [f64; 20] = [
    7.0360141451629843e-01,
    5.3267182521620393e-01,
    8.4871398060292014e-01,
    4.8644235145828230e-01,
    6.2591225797416061e-01,
    6.1096460207975911e-01,
    8.5754000210076686e-01,
    6.9157382444592275e-01,
    3.5894681558768954e-01,
    3.9470606771471589e-01,
    3.0863205069850497e-01,
    6.0118343195266277e-01,
    1.3224466930429607e-01,
    4.9000665242813629e-01,
    4.6384930499632371e-03,
    7.0031161373901474e-02,
    6.4055600294107351e-01,
    9.4184657399950988e-01,
    3.3344490739119781e-01,
    3.7870802843037710e-01,
];
const RANN_ORACLE_100: f64 = 6.5343790483526487e-01;
const RANN_ORACLE_1000: f64 = 9.7361296873358782e-01;
const RANN_ORACLE_10000: f64 = 7.1461643499877459e-01;

#[test]
fn rng_matches_gfortran_rann_oracle() {
    // The Fortran forms `iy*rm` with `rm = 1/m` pre-rounded; this port
    // divides directly, so allow an ulp or two (1e-15 relative), not
    // bit equality.
    let mut rng = Rng::new(-101);
    for (k, &want) in RANN_ORACLE_FIRST_20.iter().enumerate() {
        let got = rng.next();
        assert!(
            (got - want).abs() <= 1e-15 * want,
            "draw {}: got {got:.17e}, oracle {want:.17e}",
            k + 1
        );
    }
    let mut k = 20;
    for (target, want) in [
        (100, RANN_ORACLE_100),
        (1000, RANN_ORACLE_1000),
        (10000, RANN_ORACLE_10000),
    ] {
        let mut got = 0.0;
        while k < target {
            got = rng.next();
            k += 1;
        }
        assert!(
            (got - want).abs() <= 1e-15 * want,
            "draw {target}: got {got:.17e}, oracle {want:.17e}"
        );
    }
}

#[test]
fn rng_never_returns_zero_and_stays_in_unit_interval() {
    let mut rng = Rng::new(-7);
    for _ in 0..200_000 {
        let r = rng.next();
        assert!(r > 0.0 && r < 1.0, "{r}");
    }
}

fn plain_sequence(dbar: f64) -> SequenceLadderParams {
    SequenceLadderParams {
        dbar,
        gn_mean: 1.0e-3,
        gf_mean: 0.0,
        gg_mean: 2.3e-2,
        gx_mean: 0.0,
        ndf_n: 1,
        ndf_f: 0,
        ndf_x: 0,
        csz: 1.0,
        cth_ref: 1.0,
        cc2p: 1.0,
        cs2p: 0.0,
    }
}

#[test]
fn ladder_spacing_has_the_wigner_mean_and_width() {
    // E_{r+1} - E_r = D·√(4/π)·√(-ln U) has mean D and standard
    // deviation D·√(4/π - 1) ≈ 0.5227·D (Wigner surmise, exactly).
    let dbar = 20.0;
    let seq = plain_sequence(dbar);
    let mut rng = Rng::new(-101);
    let ladder = generate_ladder(&seq, 0.0, 400_000.0, &mut rng);
    let spacings: Vec<f64> = ladder
        .windows(2)
        .map(|w| w[1].energy - w[0].energy)
        .collect();
    let n = spacings.len() as f64;
    assert!(n > 15_000.0, "only {n} spacings");
    let mean = spacings.iter().sum::<f64>() / n;
    let var = spacings.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let want_sd = dbar * (4.0 / std::f64::consts::PI - 1.0).sqrt();
    // Statistical error of the mean: sd/√n ≈ 0.075 eV here; 4σ bands.
    assert!(
        (mean - dbar).abs() < 4.0 * want_sd / n.sqrt(),
        "mean {mean} vs {dbar}"
    );
    assert!(
        (var.sqrt() - want_sd).abs() < 0.02 * want_sd,
        "sd {} vs {want_sd}",
        var.sqrt()
    );
    // First resonance uniform in [elow, elow + D√(4/π)).
    assert!(
        ladder[0].energy >= 0.0 && ladder[0].energy < dbar * (4.0 / std::f64::consts::PI).sqrt()
    );
    // Ladder ends with the first resonance past ehigh, and every width
    // fraction sums to one.
    assert!(ladder.last().unwrap().energy > 400_000.0);
    assert!(ladder[..ladder.len() - 1]
        .iter()
        .all(|r| r.energy <= 400_000.0));
    for r in &ladder {
        let s = r.gn_frac + r.gf_frac + r.gg_frac + r.gx_frac;
        assert!((s - 1.0).abs() < 1e-12, "{s}");
    }
}

#[test]
fn ladder_neutron_widths_average_to_the_mean_width() {
    // ⟨Γn⟩ over the 20 χ²₁ quantile-bin multipliers is ⟨χ²₁⟩ = 1 to the
    // table's own discretisation (the tabulated bin means of `chisq`
    // average to 0.9996 for ν=1 — an upstream property, kept as-is).
    let seq = plain_sequence(1.0);
    let mut rng = Rng::new(-101);
    let ladder = generate_ladder(&seq, 0.0, 50_000.0, &mut rng);
    let n = ladder.len() as f64;
    let mean_gn = ladder
        .iter()
        .map(|r| r.gn_frac * r.total_width)
        .sum::<f64>()
        / n;
    let table_mean: f64 = CHISQ.iter().map(|row| row[0]).sum::<f64>() / 20.0;
    assert!(
        (mean_gn / seq.gn_mean - table_mean).abs() < 0.03,
        "{mean_gn} vs {table_mean}"
    );
    // Capture width is never fluctuated.
    for r in &ladder {
        assert!(((r.gg_frac * r.total_width) - seq.gg_mean).abs() < 1e-12);
    }
}

/// Reference `w(x, y)` for the tier test: the full series evaluator.
fn exact(x: f64, y: f64) -> (f64, f64) {
    wfun::uw2(x, y)
}

fn rel_err(got: (f64, f64), want: (f64, f64)) -> f64 {
    let er = (got.0 - want.0).abs() / want.0.abs();
    let ei = (got.1 - want.1).abs() / want.1.abs().max(1e-3 * want.0.abs());
    er.max(ei)
}

#[test]
fn line_shape_tiers_reproduce_uw2_to_each_approximations_accuracy() {
    // Points chosen to sit on both sides of every tier boundary (100, 6,
    // 3.9/3.0, 0.5) plus interior points. Each tier's tolerance is the
    // accuracy that approximation is *supposed* to deliver (measured
    // 2026-09-10 against uw2; the bounds carry ~2× margin):
    //   asymptotic (|x|>100 or y>100)   : 1e-3
    //   2-term rational (>6, ≤100)      : 5e-3
    //   3-term rational (>3.9/3.0, ≤6)  : 5e-3
    //   table (≤3.9, ≤3.0)              : 5e-3 (fine grid near y→0 is the worst)
    let table = DopplerTable::new();
    let cases: &[(f64, f64, f64)] = &[
        // asymptotic
        (150.0, 0.7, 1e-3),
        (-120.0, 2.0, 1e-3),
        (0.1, 120.0, 1e-3),
        (100.5, 0.0, 1e-3),
        // 2-term rational: |x| in (6,100] or y in (6,100]
        (50.0, 2.0, 5e-3),
        (10.0, 0.3, 5e-3),
        (6.1, 0.1, 5e-3),
        (-7.0, 5.0, 5e-3),
        (1.0, 6.1, 5e-3),
        (99.0, 99.0, 5e-3),
        // 3-term rational: |x| in (3.9,6] or y in (3.0,6]
        (4.0, 0.01, 5e-3),
        (5.9, 2.0, 5e-3),
        (-4.5, 0.4, 5e-3),
        (0.5, 3.1, 5e-3),
        (3.0, 5.9, 5e-3),
        // table, coarse (y ≥ 0.5)
        (0.0, 0.5, 5e-3),
        (0.37, 0.5, 5e-3),
        (3.85, 2.95, 5e-3),
        (2.5, 2.9, 5e-3),
        (-1.25, 1.55, 5e-3),
        // table, fine (y < 0.5)
        (0.0, 0.49, 5e-3),
        (1.0, 0.05, 5e-3),
        (3.85, 0.45, 5e-3),
        (3.0, 0.4, 5e-3),
        (-0.33, 0.21, 5e-3),
        (2.13, 0.001, 5e-3),
    ];
    for &(x, y, tol) in cases {
        let got = line_shape(x, y, y * y, &table);
        let want = exact(x, y);
        let e = rel_err(got, want);
        assert!(
            e <= tol,
            "line_shape({x},{y}) = {got:?} vs uw2 {want:?}: rel err {e:e} > {tol:e}"
        );
    }
}

#[test]
fn line_shape_selects_the_same_tier_as_upstreams_index_ranges() {
    // Tier selection is observable: each closed-form tier is a specific
    // formula. Evaluate those formulas directly and check `line_shape`
    // returns exactly one of them (bit-for-bit) on the expected side of
    // each boundary, and the table on the other side.
    let table = DopplerTable::new();
    let asym = |x: f64, y: f64| {
        let a1 = 0.5641895835 / (x * x + y * y);
        (y * a1, x * a1)
    };
    let two_term = |x: f64, y: f64| {
        let (yy, a1, a2) = (y * y, x * x - y * y, 2.0 * x * y);
        let a3 = a2 * a2;
        let (temp1, temp2) = (a2 * x, a2 * y);
        let (a4, a5) = (a1 - 0.2752551, a1 - 2.724745);
        let f1 = 0.5124242 / (a4 * a4 + a3);
        let f2 = 0.05176536 / (a5 * a5 + a3);
        let _ = yy;
        (
            f1 * (temp1 - a4 * y) + f2 * (temp1 - a5 * y),
            f1 * (a4 * x + temp2) + f2 * (a5 * x + temp2),
        )
    };
    // Just past 100 → asymptotic; just under → 2-term.
    assert_eq!(line_shape(100.001, 1.0, 1.0, &table), asym(100.001, 1.0));
    assert_eq!(
        line_shape(1.0, 100.001, 100.001f64.powi(2), &table),
        asym(1.0, 100.001)
    );
    assert_eq!(line_shape(99.999, 1.0, 1.0, &table), two_term(99.999, 1.0));
    // Just past 6 → 2-term; y alone past 6 too.
    assert_eq!(line_shape(6.001, 0.2, 0.04, &table), two_term(6.001, 0.2));
    assert_eq!(
        line_shape(0.2, 6.001, 6.001f64.powi(2), &table),
        two_term(0.2, 6.001)
    );
    // Under 6 but past 3.9 (x) / 3.0 (y): 3-term, i.e. NOT the 2-term
    // formula and NOT the table (differs from both).
    let g = line_shape(5.999, 0.2, 0.04, &table);
    assert_ne!(g, two_term(5.999, 0.2));
    let g2 = line_shape(3.95, 0.2, 0.04, &table);
    assert_ne!(g2, table.lookup_fine(3.95, 0.2));
    let g3 = line_shape(0.2, 3.05, 3.05f64.powi(2), &table);
    assert_ne!(g3, table.lookup_coarse(0.2, 3.05));
    // Inside the table region: exactly the table, coarse vs fine by y.
    assert_eq!(
        line_shape(3.85, 2.95, 2.95f64.powi(2), &table),
        table.lookup_coarse(3.85, 2.95)
    );
    assert_eq!(
        line_shape(3.85, 0.5, 0.25, &table),
        table.lookup_coarse(3.85, 0.5)
    );
    assert_eq!(
        line_shape(3.85, 0.499, 0.499f64.powi(2), &table),
        table.lookup_fine(3.85, 0.499)
    );
    assert_eq!(
        line_shape(-1.0, 0.1, 0.01, &table),
        table.lookup_fine(-1.0, 0.1)
    );
}
