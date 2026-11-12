//! Function-level differential tests for the incident-charged-particle ACER
//! routines against NJOY2016's own compiled routines (gh:#1485, epic #1336).
//!
//! **Methodology.** `verification_and_validation/acer_charged/build_and_run.sh`
//! builds NJOY2016 (`acefc.f90` SHA-256 checked; the only edit is exporting
//! `ptlegc` and `coul`) and a driver (`driver.f90`) that calls, on fixed inputs,
//! `terp1` with `INT = 6` (Coulomb penetrability, `endf.f90:1636-1645`),
//! `coul` (`acefc.f90:8117-8215`) at five cosines and `ptlegc`
//! (`acefc.f90:7994-8115`) on real MF=6/MT=2 `LAW = 5` LIST records of five
//! charged-particle evaluations (`lists.txt`, from `extract_lists.py`): the
//! first, middle and last incident energy of p + H-1, p + H-2, p + H-3, d + H-2
//! and alpha + He-4. Every result is printed as a decimal and as its IEEE-754
//! bit pattern (`reference_output.txt`, gfortran 13.3, `-O3`).
//!
//! **Pass criterion.** Bit-identical results, and the same number and the same
//! bits of `ptlegc` points. Round-off-level differences are reported if they
//! occur (the workspace's code-to-code allowance, < 1e-8 relative with an
//! understood cause), never absorbed silently.
//!
//! **Result (2026-11-11, rustc 1.99.1, PETIR maths against glibc).** See the
//! assertions' messages; the counts are printed with `--nocapture`.

use njoy_outram_park_fork::acer::charged_coulomb::{coul, ptlegc};
use njoy_outram_park_fork::endf::interp::{terp1, IntLaw};

const LISTS: &str = include_str!("../verification_and_validation/acer_charged/lists.txt");
const REFERENCE: &str = include_str!("../verification_and_validation/acer_charged/reference_output.txt");

struct Case {
    label: String,
    awp: f64,
    izai: i32,
    awt: f64,
    iza: i32,
    spi: f64,
    c: Vec<f64>,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    let mut lines = LISTS.lines();
    while let Some(h) = lines.next() {
        let f: Vec<&str> = h.split_whitespace().collect();
        let data: Vec<f64> = lines.next().unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
        assert_eq!(data.len(), f[6].parse::<usize>().unwrap());
        // The driver's `c(2000)` is zero beyond the record, and `coul` reads
        // past a short record (the formula wants more words than NW gives for
        // the first energy of some tapes); pad the same way.
        let mut data = data;
        data.resize(2000, 0.0);
        out.push(Case {
            label: f[0].to_string(),
            awp: f[1].parse().unwrap(),
            izai: f[2].parse().unwrap(),
            awt: f[3].parse().unwrap(),
            iza: f[4].parse().unwrap(),
            spi: f[5].parse().unwrap(),
            c: data,
        });
    }
    out
}

fn bits(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).unwrap())
}

/// `terp1` with `INT = 6` at six points, one of them the `y1 = 0` guard.
/// Pass criterion: bit-identical.
#[test]
fn terp1_int6_matches_njoy2016() {
    let mut n = 0;
    let mut same = 0;
    for l in REFERENCE.lines().filter(|l| l.starts_with("TERP6")) {
        let f: Vec<&str> = l.split_whitespace().collect();
        let i: usize = f[1].parse().unwrap();
        let (x1, y1, x2, y2) = (1.0e3, 1.0e-3, 1.0e5, 5.0);
        let (y1, x) = match i {
            1 => (y1, 2.0e3),
            2 => (y1, 1.0e4),
            3 => (y1, 5.0e4),
            4 => (y1, 1.0e3),
            5 => (y1, 1.0e5),
            _ => (0.0, 2.0e3),
        };
        let ours = terp1(x1, y1, x2, y2, x, IntLaw::Coulomb).unwrap();
        let theirs = bits(f[3]);
        n += 1;
        if ours.to_bits() == theirs.to_bits() {
            same += 1;
        } else {
            println!("TERP6 {i}: ours {ours:e} njoy {theirs:e}");
        }
    }
    println!("terp1 INT=6: {same}/{n} bit-identical");
    assert_eq!(n, 6);
    assert_eq!(same, n);
}

/// `coul` at cosines -0.96, -0.5, 0, 0.5, 0.96 on the 9 records (LTP = 1 only;
/// LIDP = 0 and 1; no held evaluation has an LTP = 2 record). Pass criterion: bit-identical.
#[test]
fn coul_matches_njoy2016() {
    let cs = cases();
    let xs = [-0.96, -0.5, 0.0, 0.5, 0.96];
    let (mut n, mut same, mut worst) = (0, 0, 0.0f64);
    for l in REFERENCE.lines().filter(|l| l.starts_with("COUL")) {
        let f: Vec<&str> = l.split_whitespace().collect();
        let label = f[1];
        let k: usize = f[2].parse().unwrap();
        // the records of one label appear in `lists.txt` order; count them
        let idx = REFERENCE.lines().filter(|l| l.starts_with("COUL")).take_while(|m| !std::ptr::eq(*m, l)).count() / 5;
        let c = &cs[idx];
        assert_eq!(c.label, label);
        let ours = coul(xs[k - 1], &c.c, c.awp, c.izai, c.awt, c.iza, c.spi);
        let theirs = bits(f[4]);
        n += 1;
        if ours.to_bits() == theirs.to_bits() {
            same += 1;
        } else {
            let rel = ((ours - theirs) / theirs).abs();
            worst = worst.max(rel);
            println!("COUL {label} x={}: ours {ours:e} njoy {theirs:e} rel {rel:e}", xs[k - 1]);
        }
    }
    println!("coul: {same}/{n} bit-identical, worst relative difference {worst:e}");
    assert_eq!(n, 45);
    // Two last-ulp differences (d + H-2 at 1.4 MeV, LTP = 1, LIDP = 1,
    // x = +-0.5: 1.52e-16) remain. Their cause is the maths library, not the
    // port: `cos(eta ln((1+x)/2))` at x = -0.5 (and its mirror at +0.5) is
    // rounded differently by PETIR's `cos` (the `libm` crate, kept on purpose,
    // maintainer 2026-11-11, gh:#1455) and by glibc's, which the Fortran
    // called; see `identical_particle_cosine_differs_only_in_libm` below.
    // Pinned, not loosened: exactly these two, nothing else, under 2e-16.
    assert_eq!(same, 43, "worst relative difference {worst:e}");
    assert!(worst < 2.0e-16);
}

/// The proof that the two `coul` differences are libm and nothing else: for
/// the d + H-2 record at 1.4 MeV, `coul`'s own `eta`, the arguments
/// `eta ln((1-x)/2)` and `eta ln((1+x)/2)` of its LTP = 1, LIDP = 1 branch
/// (`acefc.f90:8195-8203`) go to PETIR's `cos`/`sin`/`ln` and to glibc's
/// (Rust's `f64::cos` etc. call the system libm, which the Fortran also
/// called), and at least one differs in the last bit. A Python transcription
/// of the same expression with glibc maths gives NJOY's bits exactly
/// (`0x3FD75F5020806155`, recorded in the V&V README).
#[test]
fn identical_particle_cosine_differs_only_in_libm() {
    use njoy_outram_park_fork::common::phys::{AMASSN_AMU as AMASSN, AMU_G as AMU, CLIGHT_CM_S as CLIGHT, EV_ERG as EV, HBAR_ERG_S as HBAR};
    use petir::mathf::RealMath;
    let cs = cases();
    // the E = 1.4 MeV record (the middle one), where both residuals sit
    let c = cs.iter().filter(|c| c.label == "deck53_d_H2").nth(1).unwrap();
    let (ai, e) = (c.awp * AMASSN, c.c[1]);
    let ee = (EV / 10_000_000.0) * (CLIGHT / 10.0);
    let cc2 = ee.r_powi(4) * AMU / (2.0 * HBAR.r_powi(2) * EV);
    let eta = f64::from(c.iza / 1000) * f64::from(c.izai / 1000) * (cc2 * ai / e).sqrt();
    let mut differing = 0;
    // LTP = 1, LIDP = 1 (`acefc.f90:8190-8212`): th1 = eta ln((1-x)/2),
    // th2 = eta ln((1+x)/2); cos and sin of both.
    for x in [0.5f64, -0.5] {
        for r in [(1.0 - x) / 2.0, (1.0 + x) / 2.0] {
            let (ln_p, ln_g) = (r.r_ln(), r.ln());
            let (th_p, th_g) = (eta * ln_p, eta * ln_g);
            let pairs = [("ln", ln_p, ln_g), ("cos", th_p.r_cos(), th_g.cos()), ("sin", th_p.r_sin(), th_g.sin())];
            for (name, p, g) in pairs {
                if p.to_bits() != g.to_bits() {
                    differing += 1;
                    println!("x={x} r={r}: {name} petir {p:e} glibc {g:e}");
                }
            }
        }
    }
    println!("PETIR vs this platform's libm: {differing} of 12 values differ in the last bit");
    // The claim is about glibc, the libm the Fortran reference called
    // (gfortran 13.3, Linux). On macOS (Apple libm) and Windows (MSVC CRT) the
    // platform libm is a different library, and CI 2026-11-12 (run 43185822373)
    // showed it rounds these four arguments the same way PETIR does, so
    // `differing == 0` there says nothing about the glibc explanation. Assert
    // only where the premise holds; the `coul` pin above (43 of 45
    // bit-identical, worst < 2e-16) is asserted on every platform.
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        assert!(differing > 0, "PETIR and glibc agree here: the coul residual would then be unexplained");
    }
}

/// `ptlegc`'s adaptive reconstruction and thinning on the same 9 records.
/// Pass criterion: the same number of points, every abscissa and ordinate
/// bit-identical.
#[test]
fn ptlegc_matches_njoy2016() {
    let cs = cases();
    let mut lines = REFERENCE.lines().peekable();
    let (mut npts, mut same) = (0, 0);
    let mut idx = 0;
    while let Some(l) = lines.next() {
        if !l.starts_with("PTLEGC") {
            continue;
        }
        let f: Vec<&str> = l.split_whitespace().collect();
        let n: usize = f[3].parse().unwrap();
        let c = &cs[idx];
        assert_eq!(c.label, f[1]);
        idx += 1;
        let ours = ptlegc(&c.c, c.awp, c.izai, c.awt, c.iza, c.spi).unwrap();
        assert_eq!(ours.len(), n, "{}: point count", c.label);
        for (i, pt) in ours.iter().enumerate() {
            let g: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            let (a, b) = (bits(g[0]), bits(g[2]));
            npts += 1;
            if pt.0.to_bits() == a.to_bits() && pt.1.to_bits() == b.to_bits() {
                same += 1;
            } else {
                println!("PTLEGC {} point {}: ours ({:e}, {:e}) njoy ({a:e}, {b:e})", c.label, i + 1, pt.0, pt.1);
            }
        }
    }
    println!("ptlegc: {idx} records, {same}/{npts} points bit-identical");
    assert_eq!(idx, 9);
    assert_eq!(same, npts);
}
