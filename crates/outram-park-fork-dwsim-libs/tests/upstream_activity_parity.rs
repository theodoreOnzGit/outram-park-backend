// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — NRTL and UNIQUAC activity coefficients against upstream DWSIM (gh:#1356)
//!
//! **Methodology.** Upstream `NRTL.GAMMA_MR` and `UNIQUAC.GAMMA_MR` (DWSIM
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`) are transcribed verbatim into
//! `verification_and_validation/upstream_differential_gh1356/activity_driver.py`
//! (route and its limits: that folder's README; transcription proven by
//! `check_verbatim.sh`). Ten systems (NRTL and UNIQUAC; 2, 3, 4 components;
//! temperature-independent and with `B`, `C` terms; pair parameters stored in
//! both orientations of upstream's interaction dictionary), 3 temperatures,
//! 23 compositions each: 690 points per model family. Upstream gammas are frozen
//! as raw IEEE bit patterns in `tests/fixtures/upstream_gh1356/`; this crate is
//! recomputed on every run, from the same decimal literals.
//!
//! **Bounds, derived before measuring** (README, "Predictions"): gate 1e-13
//! relative on gamma; bit-identity is the goal, 1-ulp PETIR-vs-libm `exp`/`ln`
//! differences are accepted but counted and pinned.
//!
//! **Results (2026-11-13, release; 690 points, 1932 gammas).**
//!
//! | model | before the fix | after |
//! |---|---|---|
//! | NRTL | 866/966 bit-identical, worst 4.0e-16 (2 ulp) | 964/966, worst 1.5e-16 (1 ulp, PETIR vs libm) |
//! | UNIQUAC | 313/966 bit-identical, worst 2.0e-15 (12 ulp) | 966/966, 0 ulp |

use outram_park_fork_dwsim_libs::thermo::activity::{ActivityModel, NrtlParams, UniquacParams};

/// One fixture row: model kind, system id, T, x, upstream gamma.
struct Row {
    kind: String,
    sys: String,
    t: f64,
    x: Vec<f64>,
    gamma: Vec<f64>,
}

fn f(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("hex bits"))
}

fn rows() -> Vec<Row> {
    let text = include_str!("fixtures/upstream_gh1356/activity_upstream_output.txt");
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        let head: Vec<&str> = parts[0].split_whitespace().collect();
        out.push(Row {
            kind: head[0].to_string(),
            sys: head[1].to_string(),
            t: f(head[2]),
            x: head[3..].iter().map(|h| f(h)).collect(),
            gamma: parts[1].split_whitespace().map(f).collect(),
        });
    }
    out
}

/// A binary pair as upstream stores it: `(p, q, A12, A21, B12, B21, C12, C21, alpha)`.
type Pair = (usize, usize, f64, f64, f64, f64, f64, f64, f64);

fn matrices(n: usize, pairs: &[Pair]) -> [Vec<Vec<f64>>; 4] {
    let z = || vec![vec![0.0; n]; n];
    let (mut a, mut b, mut c, mut al) = (z(), z(), z(), z());
    for &(p, q, a12, a21, b12, b21, c12, c21, alpha) in pairs {
        a[p][q] = a12;
        a[q][p] = a21;
        b[p][q] = b12;
        b[q][p] = b21;
        c[p][q] = c12;
        c[q][p] = c21;
        al[p][q] = alpha;
        al[q][p] = alpha;
    }
    [a, b, c, al]
}

// ids: A=0 B=1 C=2 D=3 ; W=0 E=1. Literals are identical to activity_driver.py.
fn nrtl_system(name: &str) -> NrtlParams {
    let (n, pairs): (usize, Vec<Pair>) = match name {
        "n2" => (2, vec![(0, 1, -120.5, 950.25, 0.0, 0.0, 0.0, 0.0, 0.3)]),
        "n2t" => (2, vec![(0, 1, -120.5, 950.25, 0.35, -0.82, 1.1e-4, -2.3e-4, 0.3)]),
        "n3" => (
            3,
            vec![
                (0, 1, 300.5, -150.25, 0.0, 0.0, 0.0, 0.0, 0.3),
                (2, 0, 1200.0, 80.5, 0.0, 0.0, 0.0, 0.0, 0.2),
                (1, 2, -75.0, 640.75, 0.0, 0.0, 0.0, 0.0, 0.47),
            ],
        ),
        "n3t" => (
            3,
            vec![
                (0, 1, 300.5, -150.25, 0.12, -0.2, 3.0e-5, 1.0e-4, 0.3),
                (2, 0, 1200.0, 80.5, -0.9, 0.4, -2e-4, 5e-5, 0.2),
                (1, 2, -75.0, 640.75, 0.05, 0.7, 1e-4, -1e-4, 0.47),
            ],
        ),
        "n4" => (
            4,
            vec![
                (0, 1, 300.5, -150.25, 0.0, 0.0, 0.0, 0.0, 0.3),
                (0, 2, 1200.0, 80.5, 0.0, 0.0, 0.0, 0.0, 0.2),
                (3, 0, 15.5, 22.25, 0.0, 0.0, 0.0, 0.0, 0.35),
                (1, 2, -75.0, 640.75, 0.0, 0.0, 0.0, 0.0, 0.47),
                (1, 3, 410.0, -33.0, 0.3, 0.1, 0.0, 0.0, 0.25),
                (3, 2, -210.0, 512.0, 0.0, 0.0, 0.0, 0.0, 0.4),
            ],
        ),
        other => panic!("unknown NRTL system {other}"),
    };
    let [a, b, c, al] = matrices(n, &pairs);
    NrtlParams::new(a, b, c, al)
}

fn uniquac_system(name: &str) -> UniquacParams {
    let (r, q, pairs): (Vec<f64>, Vec<f64>, Vec<Pair>) = match name {
        "u2" => (
            vec![4.0, 2.1],
            vec![3.0, 1.97],
            vec![(0, 1, -95.5, 410.25, 0.0, 0.0, 0.0, 0.0, 0.0)],
        ),
        "u2t" => (
            vec![4.0, 2.1],
            vec![3.0, 1.97],
            vec![(0, 1, -95.5, 410.25, 0.31, -0.42, 1.2e-4, -3.1e-4, 0.0)],
        ),
        "u3" => (
            vec![3.19, 4.06, 1.43],
            vec![2.97, 3.24, 1.4],
            vec![
                (0, 1, 300.5, -150.25, 0.0, 0.0, 0.0, 0.0, 0.0),
                (2, 0, -90.0, 80.5, 0.0, 0.0, 0.0, 0.0, 0.0),
                (1, 2, -75.0, 640.75, 0.0, 0.0, 0.0, 0.0, 0.0),
            ],
        ),
        "u3t" => (
            vec![3.19, 4.06, 1.43],
            vec![2.97, 3.24, 1.4],
            vec![
                (0, 1, 300.5, -150.25, 0.12, -0.2, 3.0e-5, 1.0e-4, 0.0),
                (2, 0, -90.0, 80.5, -0.9, 0.4, -2e-4, 5e-5, 0.0),
                (1, 2, -75.0, 640.75, 0.05, 0.7, 1e-4, -1e-4, 0.0),
            ],
        ),
        "u4" => (
            vec![3.19, 4.06, 1.43, 2.5],
            vec![2.97, 3.24, 1.4, 2.2],
            vec![
                (0, 1, 300.5, -150.25, 0.0, 0.0, 0.0, 0.0, 0.0),
                (0, 2, -90.0, 80.5, 0.0, 0.0, 0.0, 0.0, 0.0),
                (3, 0, 15.5, 22.25, 0.0, 0.0, 0.0, 0.0, 0.0),
                (1, 2, -75.0, 640.75, 0.0, 0.0, 0.0, 0.0, 0.0),
                (1, 3, 410.0, -33.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                (3, 2, -210.0, 512.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            ],
        ),
        other => panic!("unknown UNIQUAC system {other}"),
    };
    let n = r.len();
    let [a, b, c, _] = matrices(n, &pairs);
    UniquacParams::new(r, q, a, b, c)
}

/// Distance in units in the last place between two finite doubles of one sign.
fn ulps(a: f64, b: f64) -> u64 {
    (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs()
}

/// `(points, components compared, bit-identical components, worst relative gap, worst ulp)`
fn compare(kind: &str) -> (usize, usize, usize, f64, u64) {
    let (mut pts, mut comps, mut same, mut worst, mut worst_ulp) = (0, 0, 0, 0.0_f64, 0_u64);
    for row in rows().into_iter().filter(|r| r.kind == kind) {
        let model = if kind == "NRTL" {
            ActivityModel::Nrtl(nrtl_system(&row.sys))
        } else {
            ActivityModel::Uniquac(uniquac_system(&row.sys))
        };
        let g = model.activity_coefficients(&row.x, row.t);
        pts += 1;
        for (mine, theirs) in g.iter().zip(&row.gamma) {
            comps += 1;
            if mine.to_bits() == theirs.to_bits() {
                same += 1;
            }
            worst = worst.max(((mine - theirs) / theirs).abs());
            worst_ulp = worst_ulp.max(ulps(*mine, *theirs));
        }
    }
    (pts, comps, same, worst, worst_ulp)
}

#[test]
fn nrtl_matches_upstream_gamma_mr() {
    let (pts, comps, same, worst, worst_ulp) = compare("NRTL");
    println!("NRTL: {pts} points, {same}/{comps} bit-identical, worst rel {worst:e}, worst {worst_ulp} ulp");
    assert_eq!(pts, 345, "fixture row count");
    assert!(worst < 1e-13, "gate 1e-13 breached: {worst:e}");
    // Pinned 2026-11-13. Before the operation-order fix (gh:#1356): 866/966, 2 ulp.
    // The 2 remaining 1-ulp components are PETIR-vs-libm exp/ln.
    assert_eq!(same, 964, "bit-identical count moved");
    assert!(worst_ulp <= 1);
}

#[test]
fn uniquac_matches_upstream_gamma_mr() {
    let (pts, comps, same, worst, worst_ulp) = compare("UNIQUAC");
    println!("UNIQUAC: {pts} points, {same}/{comps} bit-identical, worst rel {worst:e}, worst {worst_ulp} ulp");
    assert_eq!(pts, 345, "fixture row count");
    assert!(worst < 1e-13, "gate 1e-13 breached: {worst:e}");
    // Pinned 2026-11-13. Before the operation-order fix (gh:#1356): 313/966, 12 ulp,
    // worst relative 2.0e-15.
    assert_eq!(same, comps, "bit-identical count moved");
}
