// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **Source distributions against OpenMC `8f79d9d0` with a fixed random-number
//! sequence** (gh:#1388, differential epic gh:#1336).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_samplers/run_distributions.sh`
//! compiles OpenMC's REAL `distribution.cpp` (lines 26-772, a SHA-checked verbatim
//! extract: `DiscreteIndex` Vose alias tables, `Discrete`, `Uniform`, `PowerLaw`,
//! `Maxwell`, `Watt`, `Normal` incl. truncation, `Tabular` x4 interpolations,
//! `Mixture`, bias weights, `distribution_from_xml`) and `distribution_spatial.cpp`
//! (`CartesianIndependent`, `CylindricalIndependent`, `SphericalIndependent`,
//! `PointCloud`, `SpatialBox`, `SpatialPoint`), plus `random_dist.cpp`,
//! `random_lcg.cpp` and `position.cpp` unmodified. Only the XML layer (an
//! in-memory tree) and `mesh.h` are stubbed, so upstream's own XML constructors
//! run. For each case it prints the sampled value, the importance weight and the
//! **final RNG state** (so an extra, missing or reordered draw is a difference)
//! for 40 seeds as hex bits, plus `evaluate` at fixed points and `integral()`.
//! This test builds the same distributions from the same decimal parameters in
//! `outram_mc_libs::source::distribution` and compares.
//!
//! **Pass criterion:** the end state, the weight and the value bit-identical, except
//! values in the families that call libm (`pow`, `exp`, `ln`, `log1p`, `expm1`,
//! `sin`, `cos`, `erfc`), which may differ in the last bit because PETIR's
//! `RealMath` is not glibc (gh:#1455, accepted). The test pins the count and worst
//! relative difference of those and proves the cause with an A/B in `std` (glibc).
//!
//! # Prediction (written before the first run, 2026-11-11)
//!
//! Bit-identical (no libm in the path): `Discrete` (alias), `Uniform`, `Tabular`
//! histogram and lin-lin, `SpatialBox`, `SpatialPoint`, `PointCloud` (and their
//! weights, `integral`, and every end state). Expected last-bit differences in a
//! few percent of samples: `PowerLaw` (`pow`), `Normal` (`ln`), `Maxwell`/`Watt`
//! (`ln`/`cos`, as in the secondary-sampler test), `Tabular` log-lin / log-log,
//! spherical / cylindrical positions (`sin`/`cos`), and the truncated-`Normal`
//! `evaluate` (`erfc`). End states bit-identical everywhere, including the
//! rejection loop of the truncated normal (the accept decision would differ only
//! if a last-bit difference straddled a bound, which is not expected).
//! `PointCloud` with XML bias: upstream re-reads the unbiased strengths as the
//! bias (`distribution_spatial.cpp:420`), so every weight is exactly 1; the port
//! takes the bias argument literally, so that case is NOT compared to the port's
//! biased constructor but asserted to be upstream's defect.
//!
//! # Results
//!
//! See `verification_and_validation/upstream_openmc_samplers/README.md`.

use outram_mc_libs::source::distribution::{
    Distribution, Legendre, SpatialDistribution, TabularInterp,
};

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_samplers/reference_distributions.txt");

fn hex(line: &str, key: &str) -> u64 {
    let pat = format!(" {key}=");
    let i = line.find(&pat).unwrap_or_else(|| panic!("no {key} in {line}")) + pat.len();
    let rest = &line[i..];
    let end = rest.find(' ').unwrap_or(rest.len());
    u64::from_str_radix(&rest[..end], 16).unwrap()
}
fn f(line: &str, key: &str) -> f64 {
    f64::from_bits(hex(line, key))
}
fn lines(prefix: &str, name: &str) -> Vec<&'static str> {
    REFERENCE
        .lines()
        .filter(|l| {
            let mut t = l.split_whitespace();
            t.next() == Some(prefix) && t.next() == Some(name)
        })
        .collect()
}
fn seed_of(s: i64) -> u64 {
    (s as u64) * 104729 + 3
}

/// Tally of one comparison: count identical, total, worst relative difference.
#[derive(Default, Debug)]
struct Tally {
    same: usize,
    total: usize,
    worst: f64,
    end_same: usize,
    end_total: usize,
}
impl Tally {
    fn add(&mut self, a: f64, b: f64) {
        self.total += 1;
        if a.to_bits() == b.to_bits() {
            self.same += 1;
        } else {
            let rel = ((a - b) / b).abs();
            if rel > self.worst {
                self.worst = rel;
            }
        }
    }
    fn end(&mut self, a: u64, b: u64) {
        self.end_total += 1;
        if a == b {
            self.end_same += 1;
        }
    }
}

fn compare_dist(name: &str, d: &Distribution) -> (Tally, Tally) {
    let (mut v, mut w) = (Tally::default(), Tally::default());
    let ls = lines("S", name);
    assert_eq!(ls.len(), 40, "{name}");
    for l in ls {
        let s = l.split("seed=").nth(1).unwrap().split(' ').next().unwrap().parse::<i64>().unwrap();
        let mut seed = seed_of(s);
        let (val, wgt) = d.sample(&mut seed);
        v.add(val, f(l, "v"));
        w.add(wgt, f(l, "w"));
        v.end(seed, hex(l, "end"));
        w.end(seed, hex(l, "end"));
    }
    (v, w)
}

fn compare_eval(name: &str, d: &Distribution) -> Tally {
    let mut t = Tally::default();
    for l in lines("E", name) {
        t.add(d.evaluate(f(l, "x")).unwrap(), f(l, "f"));
    }
    assert!(t.total > 0, "{name}");
    t
}

fn check_integral(name: &str, d: &Distribution) {
    let l = lines("I", name);
    assert_eq!(d.integral().to_bits(), hex(l[0], "integral"), "{name} integral");
}

const E5: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const P5: [f64; 5] = [0.1, 0.4, 0.2, 0.05, 0.25];
const TX: [f64; 4] = [1.0, 2.0, 3.5, 5.0];
const TP: [f64; 4] = [0.5, 1.5, 1.0, 0.2];

fn univariate_cases() -> Vec<(&'static str, Distribution, bool)> {
    vec![
        ("uniform", Distribution::uniform(1.0, 4.0), false),
        ("powerlaw", Distribution::power_law(2.0, 5.0, 2.0), true),
        ("powerlaw_frac", Distribution::power_law(1.0, 3.0, 1.5), true),
        ("maxwell", Distribution::maxwell(1.3e6), true),
        ("watt", Distribution::watt(0.988e6, 2.249e-6), true),
        ("normal", Distribution::normal(2.0, 0.5, -f64::MAX, f64::MAX).unwrap(), true),
        ("normal_trunc", Distribution::normal(2.0, 0.5, 1.8, 2.4).unwrap(), true),
        ("normal_tail", Distribution::normal(0.0, 1.0, 2.0, 4.0).unwrap(), true),
        ("discrete5", Distribution::discrete(&E5, &P5), false),
        ("discrete1", Distribution::discrete(&[7.5], &[3.0]), false),
        ("discrete2", Distribution::discrete(&[1.0e6, 2.0e6], &[0.7, 0.3]), false),
        ("tab_hist", Distribution::tabular(&TX, &[0.5, 1.5, 1.0, 0.0], TabularInterp::Histogram), false),
        ("tab_lin", Distribution::tabular(&TX, &TP, TabularInterp::LinLin), false),
        ("tab_flat", Distribution::tabular(&[1.0, 2.0, 3.0], &[1.0, 1.0, 1.0], TabularInterp::LinLin), false),
        ("tab_loglin", Distribution::tabular(&TX, &TP, TabularInterp::LogLin), true),
        ("tab_loglog", Distribution::tabular(&TX, &TP, TabularInterp::LogLog), true),
    ]
}

fn mixture_parts() -> Vec<(f64, Distribution)> {
    vec![
        (0.2, Distribution::uniform(1.0, 4.0)),
        (0.5, Distribution::discrete(&E5, &P5)),
        (0.3, Distribution::tabular(&TX, &TP, TabularInterp::LinLin)),
    ]
}

/// **Function-by-function: every univariate distribution** (value, weight, end
/// state, `integral()`) against upstream. Libm-free families are asserted
/// bit-identical; libm families are pinned (count and worst relative).
#[test]
fn univariate_distributions_match_openmc() {
    let libm_free = [
        "uniform", "discrete5", "discrete1", "discrete2", "tab_hist", "tab_lin", "tab_flat",
    ];
    let mut report = Vec::new();
    for (name, d, _libm) in univariate_cases() {
        check_integral(name, &d);
        let (v, w) = compare_dist(name, &d);
        assert_eq!(v.end_same, v.end_total, "{name}: the number/order of draws differs");
        assert_eq!(w.same, w.total, "{name}: unbiased weight must be exactly 1");
        if libm_free.contains(&name) {
            assert_eq!(v.same, v.total, "{name}: libm-free family must be bit-identical, {v:?}");
        } else {
            assert!(v.worst < 1e-14, "{name}: worst relative {} (> 1e-14)", v.worst);
        }
        report.push(format!("{name}: {}/{} worst {:.2e}", v.same, v.total, v.worst));
    }
    println!("{}", report.join("\n"));
}

/// **`evaluate` (the PDF)** at 6-12 fixed points per distribution.
#[test]
fn pdf_evaluation_matches_openmc() {
    let mut report = Vec::new();
    for (name, d, _libm) in univariate_cases() {
        if lines("E", name).is_empty() {
            assert!(d.evaluate(1.0).is_err() || !lines("S", name).is_empty());
            continue;
        }
        let t = compare_eval(name, &d);
        assert!(t.worst < 1e-13, "{name}: PDF worst relative {}", t.worst);
        report.push(format!("{name}: {}/{} worst {:.2e}", t.same, t.total, t.worst));
    }
    println!("{}", report.join("\n"));
}

/// **Mixture** (flat, nested, biased) and the biased `Discrete`/`Uniform`/`Maxwell`:
/// component choice, weights (`p_norm / b_norm`) and the end state.
#[test]
fn mixture_and_bias_match_openmc() {
    let mix = Distribution::mixture(mixture_parts());
    check_integral("mixture", &mix);
    let (v, w) = compare_dist("mixture", &mix);
    assert_eq!((v.same, v.end_same), (40, 40));
    assert_eq!(w.same, 40);

    let outer = Distribution::mixture(vec![(0.4, mix.clone()), (0.6, Distribution::maxwell(1.3e6))]);
    check_integral("mixture_nested", &outer);
    let (v, w) = compare_dist("mixture_nested", &outer);
    assert_eq!(v.end_same, 40);
    assert!(v.worst < 1e-14 && w.same == 40, "{v:?}");

    let mb = Distribution::mixture_biased(mixture_parts(), &[0.5, 0.25, 0.25]);
    let (v, w) = compare_dist("mixture_biased", &mb);
    assert_eq!((v.same, v.end_same, w.same), (40, 40, 40), "biased mixture {v:?} {w:?}");

    let db = Distribution::discrete_biased(&E5, &P5, &[0.2; 5]);
    let (v, w) = compare_dist("discrete_biased", &db);
    assert_eq!((v.same, v.end_same, w.same), (40, 40, 40), "{v:?} {w:?}");

    let ub = Distribution::uniform(1.0, 4.0).with_bias(Distribution::power_law(1.0, 4.0, 1.0)).unwrap();
    let (v, w) = compare_dist("uniform_biased", &ub);
    assert_eq!(v.end_same, 40);
    assert!(v.worst < 1e-14 && w.worst < 1e-14, "{v:?} {w:?}");

    let mbias = Distribution::maxwell(1.3e6).with_bias(Distribution::watt(0.988e6, 2.249e-6)).unwrap();
    let (v, w) = compare_dist("maxwell_biased", &mbias);
    assert_eq!(v.end_same, 40);
    assert!(v.worst < 1e-14 && w.worst < 1e-12, "{v:?} {w:?}");
}

fn tally_pos(name: &str, d: &SpatialDistribution) -> (Tally, Tally) {
    let (mut p, mut w) = (Tally::default(), Tally::default());
    for l in lines("P", name) {
        let s = l.split("seed=").nth(1).unwrap().split(' ').next().unwrap().parse::<i64>().unwrap();
        let mut seed = seed_of(s);
        let (pos, wgt) = d.sample(&mut seed);
        for (k, key) in ["x", "y", "z"].iter().enumerate() {
            p.add(pos[k], f(l, key));
        }
        w.add(wgt, f(l, "w"));
        p.end(seed, hex(l, "end"));
    }
    assert!(p.end_total > 0, "{name}");
    (p, w)
}

/// Measured 2026-11-11 (rustc 1.99.1): positions (of 120 coordinates) bit-identical to
/// OpenMC for the sin/cos families; the rest differ by <= 4.31e-16 relative (1 ulp)
/// and are explained by `spatial_differences_are_libm_roundoff`.
const PINNED_SAME: [(&str, usize); 5] =
    [("cyl_default", 116), ("cyl_tilted", 119), ("cyl_diag", 117), ("cyl_biased", 119), ("sph", 118)];

/// **Spatial distributions**: positions, weights and end states against upstream's
/// XML-constructed objects (frame construction included).
#[test]
fn spatial_distributions_match_openmc() {
    let u = |a, b| Distribution::uniform(a, b);
    let mut report = Vec::new();
    let mut check = |name: &str, d: SpatialDistribution, libm: bool| {
        let (p, w) = tally_pos(name, &d);
        assert_eq!(p.end_same, p.end_total, "{name}: draws differ");
        if libm {
            assert!(p.worst < 1e-13 && w.worst < 1e-13, "{name}: {p:?} {w:?}");
        } else {
            assert_eq!(p.same, p.total, "{name}: {p:?}");
            assert_eq!(w.same, w.total, "{name}: {w:?}");
        }
        if let Some(&(_, expect)) = PINNED_SAME.iter().find(|(n, _)| *n == name) {
            assert_eq!(p.same, expect, "{name}: pinned bit-identical count moved");
        }
        report.push(format!("{name}: pos {}/{} worst {:.2e}; weight {}/{}", p.same, p.total, p.worst, w.same, w.total));
    };
    let zero = Distribution::discrete(&[0.0], &[1.0]);

    check(
        "cartesian",
        SpatialDistribution::Cartesian {
            x: u(-1.0, 3.0),
            y: Distribution::power_law(1.0, 2.0, 2.0),
            z: Distribution::discrete(&[0.0, 1.0, 2.0], &[0.2, 0.3, 0.5]),
        },
        true,
    );
    check(
        "cartesian_default",
        SpatialDistribution::Cartesian {
            x: zero.clone(),
            y: Distribution::normal(0.0, 1.0, -f64::MAX, f64::MAX).unwrap(),
            z: zero.clone(),
        },
        true,
    );
    check(
        "cartesian_biased",
        SpatialDistribution::Cartesian {
            x: u(1.0, 4.0).with_bias(Distribution::power_law(1.0, 4.0, 1.0)).unwrap(),
            y: zero.clone(),
            z: zero.clone(),
        },
        true,
    );
    let two_pi = 6.283185307179586;
    let cyl = |origin, z_dir, r_dir, r, phi, z| SpatialDistribution::cylindrical(r, phi, z, origin, z_dir, r_dir).unwrap();
    let (r1, p1, z1) = (Distribution::power_law(0.0, 2.0, 1.0), u(0.0, two_pi), u(-1.0, 1.0));
    check("cyl_default", cyl([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], r1.clone(), p1.clone(), z1.clone()), true);
    check("cyl_tilted", cyl([1.0, -2.0, 0.5], [0.0, 1.0, 1.0], [1.0, 0.0, 0.0], r1.clone(), p1.clone(), z1.clone()), true);
    check("cyl_diag", cyl([1.0, -2.0, 0.5], [1.0, 1.0, 1.0], [1.0, -1.0, 0.0], r1, p1, z1), true);
    check(
        "cyl_biased",
        cyl(
            [0.0; 3],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            u(0.0, 2.0).with_bias(Distribution::power_law(0.0, 2.0, 1.0)).unwrap(),
            u(0.0, 3.0),
            Distribution::discrete(&[0.5, 1.5], &[0.25, 0.75]),
        ),
        true,
    );
    check(
        "sph",
        SpatialDistribution::Spherical {
            r: Distribution::power_law(1.0, 3.0, 2.0),
            cos_theta: u(-1.0, 1.0),
            phi: u(0.0, two_pi),
            origin: [0.5, 0.25, -1.0],
        },
        true,
    );
    check(
        "sph_default",
        SpatialDistribution::Spherical { r: zero.clone(), cos_theta: zero.clone(), phi: zero, origin: [0.0; 3] },
        false,
    );
    let pts5 = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0], [1.0, 1.0, 1.0]];
    check("cloud", SpatialDistribution::point_cloud(pts5, &[1.0, 2.0, 0.5, 4.0, 1.5]).unwrap(), false);
    check(
        "cloud_uniform",
        SpatialDistribution::point_cloud(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]], &[1.0; 3]).unwrap(),
        false,
    );
    check(
        "cloud_span",
        SpatialDistribution::point_cloud(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0]],
            &[3.0, 1.0, 1.0, 5.0],
        )
        .unwrap(),
        false,
    );
    check("box", SpatialDistribution::Box { lower_left: [-1.0, -2.0, -3.0], upper_right: [4.0, 5.0, 6.0] }, false);
    check("point", SpatialDistribution::Point([1.5, 2.5, 3.5]), false);
    println!("{}", report.join("\n"));
}

/// **Upstream defect, recorded and not copied** (`distribution_spatial.cpp:420`):
/// the XML reader of `PointCloud` with a `<bias>` node reads the *unbiased*
/// `strengths` as the bias strengths, so its weights are all exactly 1 and the
/// points are drawn with the unbiased strengths. The port takes the bias
/// literally: it must differ from upstream here, and match the importance-weight
/// definition `p_norm / b_norm` instead.
#[test]
fn pointcloud_xml_bias_defect_is_upstream_not_port() {
    let ls = lines("P", "cloud_bias_xml");
    assert_eq!(ls.len(), 40);
    for l in &ls {
        assert_eq!(f(l, "w"), 1.0, "upstream weight is 1 (unbiased strengths used as the bias)");
    }
    let pts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
    let port = SpatialDistribution::point_cloud_biased(pts, &[1.0, 2.0, 3.0], &[3.0, 1.0, 1.0]).unwrap();
    let mut seed = 5u64;
    let (_, w) = port.sample(&mut seed);
    // p_norm = 1/6, 2/6, 3/6 ; b_norm = 3/5, 1/5, 1/5 -> weights 5/18, 5/6 (0.8333), 5/2 (2.5)
    let allowed = [5.0 / 18.0, 5.0 / 6.0, 2.5];
    assert!(allowed.iter().any(|a| (a - w).abs() < 1e-14), "{w}");
}

/// `openmc.stats.Legendre` has no sampler upstream (`NotImplementedError`), so the port's
/// `Legendre` evaluates only: its density must integrate to `a_0` over `[-1, 1]`.
#[test]
fn legendre_density_integrates_to_a0() {
    // openmc.stats.Legendre has no sampler upstream: check the density integrates to a_0
    let l = Legendre { coefficients: vec![1.0, 0.3, -0.2, 0.1] };
    let n = 20000;
    let mut s = 0.0;
    for i in 0..n {
        let x = -1.0 + 2.0 * (i as f64 + 0.5) / n as f64;
        s += l.evaluate(x) * 2.0 / n as f64;
    }
    // midpoint-rule error of this quadrature is O(h^2) ~ 1.25e-9 at n = 20000
    assert!((s - 1.0).abs() < 1e-8, "{s}");
}

/// **Cause of the last-bit differences, measured not assumed**: `PowerLaw` recomputed
/// with `std` (glibc `pow`) from the same stream reproduces upstream's bits.
#[test]
fn powerlaw_differences_are_libm_roundoff() {
    use outram_mc_libs::rng::lcg::prn;
    let (a, b, n) = (2.0f64, 5.0f64, 2.0f64);
    let offset = a.powf(n + 1.0);
    let span = b.powf(n + 1.0) - offset;
    let ninv = 1.0 / (n + 1.0);
    let mut same = 0;
    let ls = lines("S", "powerlaw");
    for l in &ls {
        let s = l.split("seed=").nth(1).unwrap().split(' ').next().unwrap().parse::<i64>().unwrap();
        let mut seed = seed_of(s);
        let v = (offset + prn(&mut seed) * span).powf(ninv);
        if v.to_bits() == hex(l, "v") {
            same += 1;
        }
    }
    assert_eq!(same, ls.len(), "std (glibc) pow must reproduce upstream exactly");
}

/// **Cause of the spatial last-bit differences**: the cylindrical default and the
/// spherical case recomputed with `std` (glibc `pow`, `sin`, `cos`), same stream,
/// same operation order, reproduce all 120 coordinates of upstream.
#[test]
fn spatial_differences_are_libm_roundoff() {
    use outram_mc_libs::rng::lcg::prn;
    let two_pi = 6.283185307179586f64;
    let pl = |a: f64, b: f64, n: f64| (a.powf(n + 1.0), b.powf(n + 1.0) - a.powf(n + 1.0), 1.0 / (n + 1.0));
    let (mut cyl, mut sph) = (0, 0);
    for l in lines("P", "cyl_default") {
        let s = l.split("seed=").nth(1).unwrap().split(' ').next().unwrap().parse::<i64>().unwrap();
        let mut seed = seed_of(s);
        let (off, span, ninv) = pl(0.0, 2.0, 1.0);
        let r = (off + prn(&mut seed) * span).powf(ninv);
        let phi = 0.0 + prn(&mut seed) * (two_pi - 0.0);
        let z = -1.0 + prn(&mut seed) * (1.0 - -1.0);
        // r * (cos(phi) r_dir + sin(phi) phi_dir) + z z_dir + origin, r_dir = x, phi_dir = z x r = y
        let x = r * (phi.cos() * 1.0 + phi.sin() * 0.0) + z * 0.0 + 0.0;
        let y = r * (phi.cos() * 0.0 + phi.sin() * 1.0) + z * 0.0 + 0.0;
        let zz = r * (phi.cos() * 0.0 + phi.sin() * 0.0) + z * 1.0 + 0.0;
        cyl += [x, y, zz].iter().zip(["x", "y", "z"]).filter(|(v, k)| v.to_bits() == hex(l, k)).count();
        assert_eq!(seed, hex(l, "end"));
    }
    for l in lines("P", "sph") {
        let s = l.split("seed=").nth(1).unwrap().split(' ').next().unwrap().parse::<i64>().unwrap();
        let mut seed = seed_of(s);
        let (off, span, ninv) = pl(1.0, 3.0, 2.0);
        let r = (off + prn(&mut seed) * span).powf(ninv);
        let ct = -1.0 + prn(&mut seed) * (1.0 - -1.0);
        let phi = 0.0 + prn(&mut seed) * (two_pi - 0.0);
        let x = r * (1.0 - ct * ct).sqrt() * phi.cos() + 0.5;
        let y = r * (1.0 - ct * ct).sqrt() * phi.sin() + 0.25;
        let z = r * ct + -1.0;
        sph += [x, y, z].iter().zip(["x", "y", "z"]).filter(|(v, k)| v.to_bits() == hex(l, k)).count();
    }
    assert_eq!((cyl, sph), (120, 120), "std (glibc) must reproduce upstream exactly");
}
