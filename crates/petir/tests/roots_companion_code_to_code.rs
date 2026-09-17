//! Code-to-code comparison of [`petir::poly::companion`] against the `roots`
//! crate 0.0.8 **compiled and executed**.
//!
//! # Methodology
//!
//! This is the same discipline the GSL suites in this crate use, applied to a
//! different upstream. `roots` 0.0.8 was built from crates.io and run on a
//! fixed set of polynomials by `reference-data/roots/roots_reference_driver.rs`;
//! its output is committed as
//! `reference-data/roots/roots-0.0.8-reference.txt` so the comparison
//! regenerates rather than being trusted. This test parses that file and runs
//! PETIR's port on the same coefficients.
//!
//! The port omits upstream's eigenvector back-substitution (see the module
//! header for why that cannot change an eigenvalue). **This test is the
//! evidence for that claim**, not the argument for it: if the omission had
//! disturbed anything, the eigenvalues would differ here.
//!
//! # What is compared, and what is not
//!
//! Only the `eigen` records. The `sturm` records in the same file document
//! upstream's `find_roots_sturm` losing roots from degree 4 upward; PETIR
//! deliberately does not port that routine, so there is nothing here to
//! compare it against. It is recorded so the decision stays checkable.
//!
//! # Results
//!
//! Measured 2026-09-15 across 15 polynomials of degrees 2 through 10, 71 real
//! roots in total. **Every one is bit-identical to upstream.** Numbers are in
//! the assertions below.

use petir::poly::companion::roots_companion;

const REFERENCE: &str = include_str!("../../../reference-data/roots/roots-0.0.8-reference.txt");

struct Case {
    name: String,
    poly: Vec<f64>,
    eigen: Vec<f64>,
}

fn parse() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut name = String::new();
    let mut poly: Vec<f64> = Vec::new();
    for line in REFERENCE.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("case ") {
            name = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("poly ") {
            poly = rest
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
        } else if line == "eigen" || line.starts_with("eigen ") {
            let rest = line.strip_prefix("eigen").unwrap_or("");
            let eigen: Vec<f64> = rest
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            cases.push(Case {
                name: name.clone(),
                poly: poly.clone(),
                eigen,
            });
        }
    }
    cases
}

/// The reference file is present, parsed, and not empty.
///
/// A silently-empty reference set would make every comparison below pass
/// vacuously, which is the classic way a code-to-code suite stops testing
/// anything.
#[test]
fn the_reference_set_is_present_and_substantial() {
    let cases = parse();
    assert!(
        cases.len() >= 15,
        "expected at least 15 reference cases, got {}",
        cases.len()
    );
    let roots: usize = cases.iter().map(|c| c.eigen.len()).sum();
    assert!(
        roots >= 71,
        "expected at least 71 reference roots, got {roots}"
    );
}

/// Every real root PETIR finds matches the one upstream found.
///
/// # Results
///
/// All 15 cases agree on the number of real roots, and **every one of the 71
/// roots is bit-identical: worst absolute difference exactly `0`**, measured
/// 2026-09-15.
///
/// This was not the expected result and is worth saying why. The port reads
/// and writes through `Matrix::get`/`set` on row-major storage where upstream
/// indexes column-major storage directly, so agreement to a few ulp would
/// have been unsurprising and sufficient. Exact agreement is stronger: it
/// means no arithmetic expression was reassociated anywhere in the QR
/// iteration, and it is conclusive evidence for the one structural claim this
/// port makes — that dropping the eigenvector back-substitution and the `v`
/// accumulator changed no eigenvalue. Had either mattered, the difference
/// would not be zero.
///
/// This puts `companion` in the same evidence class as the five PETIR
/// surfaces that are bit-identical to compiled GSL.
#[test]
fn petir_agrees_with_upstream_roots_0_0_8() {
    let cases = parse();
    let mut worst = 0.0_f64;
    let mut compared = 0usize;

    for case in &cases {
        let mut ours: Vec<f64> = roots_companion(&case.poly)
            .into_iter()
            .filter(|z| z.is_real())
            .map(|z| z.re)
            .collect();
        ours.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert_eq!(
            ours.len(),
            case.eigen.len(),
            "case {}: root count differs -- ours {:?}, upstream {:?}",
            case.name,
            ours,
            case.eigen
        );

        for (a, b) in ours.iter().zip(case.eigen.iter()) {
            let d = (a - b).abs();
            if d > worst {
                worst = d;
            }
            compared += 1;
        }
    }

    assert!(compared >= 71, "only {compared} roots compared");
    assert!(
        worst == 0.0,
        "worst difference from upstream {worst:e} over {compared} roots"
    );
}

/// The roots PETIR returns really are roots of the polynomial handed in.
///
/// # Methodology
///
/// Agreement with upstream shows the port is faithful; it does not show
/// either of them is right. This evaluates each returned real root in the
/// original polynomial by Horner and scales by the coefficient magnitude and
/// `(1 + |x|)^n`, the same residual measure
/// [`petir::poly::quartic`] uses, so the two are comparable.
///
/// # Results
///
/// **Worst scaled residual 1.207e-16**, measured 2026-09-15 over 71 roots at
/// degrees up to 10.
///
/// Interpretation: about two orders of magnitude looser than the closed-form
/// quartic's 3.655e-18 on the same measure — the price of an iterative method
/// on an ill-conditioned companion matrix, and a small one. Note this is a
/// *residual*, not a root error: the root errors in
/// `poly::companion`'s own tests reach 2.1e-9 at degree 10 while the residual
/// stays at 1e-16, which is exactly what ill-conditioning looks like. A tiny
/// residual at a badly-determined root is not accuracy.
#[test]
fn every_returned_root_satisfies_its_polynomial() {
    let cases = parse();
    let mut worst = 0.0_f64;

    for case in &cases {
        let n = case.poly.len().saturating_sub(1);
        let scale = case
            .poly
            .iter()
            .map(|c: &f64| c.abs())
            .fold(1.0_f64, f64::max);
        for z in roots_companion(&case.poly)
            .into_iter()
            .filter(|z| z.is_real())
        {
            // Horner on the descending coefficients.
            let mut v = 0.0_f64;
            for c in case.poly.iter() {
                v = v * z.re + c;
            }
            let residual = v.abs() / (scale * (1.0 + z.re.abs()).powi(n as i32));
            if residual > worst {
                worst = residual;
            }
        }
    }

    assert!(worst < 1e-14, "worst scaled residual {worst:e}");
}

/// The upstream Sturm defect is still in the reference file, and still a
/// defect.
///
/// # Why assert on a routine PETIR does not ship
///
/// Because the decision not to ship it rests on a measurement, and a
/// measurement nobody re-runs is an assertion. If a future `roots` release
/// fixes `find_roots_sturm`, regenerating the reference file will fail this
/// test, and whoever does that should then reconsider porting it.
///
/// # Results
///
/// For `prod (x - k)`, measured 2026-09-15 — `find_roots_eigen` returns every
/// root at every degree, and `find_roots_sturm` never returns more than three:
///
/// | degree | eigen | sturm |
/// |---|---|---|
/// | 2 | 2 | 2 |
/// | 3 | 3 | 3 |
/// | 4 | 4 | **3** |
/// | 5 | 5 | **3** |
/// | 6 | 6 | **3** |
/// | 7 | 7 | **1** |
/// | 8 | 8 | **3** |
/// | 9 | 9 | **1** |
/// | 10 | 10 | **1** |
///
/// Every one of those shortfalls is silent: the returned vector carries no
/// `Err` entry, so a caller has no way to tell a complete answer from a
/// partial one.
#[test]
fn the_upstream_sturm_defect_is_still_present() {
    let mut sturm_counts: Vec<(usize, usize, usize)> = Vec::new();
    let mut name = String::new();
    let mut eigen_n = 0usize;
    for line in REFERENCE.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("case ") {
            name = rest.to_string();
        } else if line == "eigen" || line.starts_with("eigen ") {
            eigen_n = line
                .strip_prefix("eigen")
                .unwrap_or("")
                .split_whitespace()
                .count();
        } else if line == "sturm" || line.starts_with("sturm ") {
            let rest = line.strip_prefix("sturm").unwrap_or("");
            if let Some(d) = name.strip_prefix("prod_x_minus_k_n") {
                if let Ok(degree) = d.parse::<usize>() {
                    sturm_counts.push((degree, eigen_n, rest.split_whitespace().count()));
                }
            }
        }
    }

    assert!(
        sturm_counts.len() >= 9,
        "expected the prod(x-k) family in the reference file"
    );
    for &(degree, eigen_n, sturm_n) in &sturm_counts {
        assert_eq!(
            eigen_n, degree,
            "degree {degree}: find_roots_eigen should find every root"
        );
        if degree >= 4 {
            assert!(
                sturm_n <= 3 && sturm_n < degree,
                "degree {degree}: find_roots_sturm returned {sturm_n} roots, which is \
                 more than the documented ceiling of 3 -- if upstream has fixed this, \
                 reconsider porting it"
            );
        }
    }
}
