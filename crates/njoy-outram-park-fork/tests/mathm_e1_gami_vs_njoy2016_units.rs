//! **V&V, unit level (gh:#1366, epic #1336)** -- `petir::expint_e1` and `petir::gamma_inc_lower`
//! (what ACER's Madland-Nix `fmn` uses, `acer/energy/madland_nix.rs`) against NJOY2016's own
//! `mathm.f90` `e1` (SLATEC `de1`) and `gami` (SLATEC `dgami`).
//!
//! ## Methodology
//! `mathm.f90` (identical in 2016.79 `ac5adf5f` and 2016.80 `ab27c641`; SHA-256 in `run.sh`) is
//! compiled UNMODIFIED with gfortran 13.3.0 `-O2` and a driver
//! (`verification_and_validation/upstream_units/mathm_e1_gami/`), which prints hex bits of
//! `e1(u)` and `gami(a,u)` for a = 0.5, 1.5, 2.5 at 779 points: u = 1e-6 * 10^(k/100), k = 0..769
//! (to 49.6) and the `e1` branch edges 1 and 4 with neighbours, 0.5, 25, 50. The port's functions
//! are evaluated at the same bit-exact abscissae. Pass criterion: relative difference <= 1e-14.
//!
//! ## Prediction (written before running)
//! At or below 1e-14 relative, **not bit-identical** (GSL algorithms against SLATEC's), as in the issue.
//!
//! ## Results (2026-11-10)
//! See the asserted constants and `verification_and_validation/upstream_units/mathm_e1_gami/README.md`.

const REF: &str = include_str!(
    "../verification_and_validation/upstream_units/mathm_e1_gami/upstream_output.txt"
);

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

struct Row {
    x: f64,
    e1: f64,
    gami: [f64; 3],
}

fn rows() -> Vec<Row> {
    REF.lines()
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(t[0], "E");
            Row { x: hex(t[1]), e1: hex(t[2]), gami: [hex(t[3]), hex(t[4]), hex(t[5])] }
        })
        .collect()
}

/// (bit-identical, total, worst rel, x at worst)
fn compare(f: impl Fn(&Row) -> (f64, f64)) -> (usize, usize, f64, f64) {
    let (mut same, mut worst, mut xw) = (0, 0.0_f64, 0.0);
    let rs = rows();
    for r in &rs {
        let (port, up) = f(r);
        if port.to_bits() == up.to_bits() {
            same += 1;
        }
        let d = rel(port, up);
        if d > worst {
            worst = d;
            xw = r.x;
        }
    }
    (same, rs.len(), worst, xw)
}

fn e1_cmp() -> (usize, usize, f64, f64) {
    compare(|r| (petir::expint_e1(r.x).expect("e1").0, r.e1))
}

fn gami_cmp(i: usize, a: f64) -> (usize, usize, f64, f64) {
    compare(|r| (petir::gamma_inc_lower(a, r.x).expect("gami").0, r.gami[i]))
}

/// `e1`: measured 686/779 bit-identical, worst relative difference 4.16e-16 (at u = 0.3467):
/// within the predicted 1e-14, and inside 1e-15. Cause of the rest: GSL's Chebyshev series
/// against SLATEC's, a last-bit difference.
#[test]
fn e1_agrees_with_slatec_de1() {
    let (same, total, worst, _) = e1_cmp();
    assert_eq!((same, total), (686, 779));
    assert!(worst <= 1e-15, "worst {worst:e}");
}

/// `gami(1.5, u)` -- the value ACER's Madland-Nix `fmn` uses. Measured 75/779 bit-identical,
/// worst 3.75e-15 (at u = 2.45e-5); gate 1e-14 (the prediction, unchanged).
#[test]
fn gami_a1p5_agrees_with_slatec_dgami() {
    let (same, total, worst, _) = gami_cmp(1, 1.5);
    assert_eq!((same, total), (75, 779));
    assert!(worst <= 1e-14, "worst {worst:e}");
}

/// `gami(0.5, u)` and `gami(2.5, u)` are NOT used by the port (ACER only needs a = 1.5); they are
/// pinned at what was measured, not at the predicted 1e-14. a = 0.5: 25/779, worst 1.77e-15.
/// a = 2.5: 37/779, worst **1.06e-14 at u = 1.45e-6, which misses the 1e-14 prediction** (small-u
/// end, where gami ~ u^a/a and the port forms Gamma(a)*P(a,u) with Gamma(a) = exp(lngamma(a));
/// that cause is a hypothesis, not tested here).
#[test]
fn gami_other_a_are_pinned_at_measured_values() {
    let (same, total, worst, _) = gami_cmp(0, 0.5);
    assert_eq!((same, total), (25, 779));
    assert!(worst <= 1.8e-15, "worst {worst:e}");
    let (same, total, worst, xw) = gami_cmp(2, 2.5);
    assert_eq!((same, total), (37, 779));
    assert!(worst <= 1.07e-14 && worst > 1.0e-14, "worst {worst:e} at {xw:e}");
}

#[test]
fn print_measurements() {
    println!("e1: {:?}", e1_cmp());
    for (i, a) in [0.5, 1.5, 2.5].iter().enumerate() {
        println!("gami a={a}: {:?}", gami_cmp(i, *a));
    }
}
