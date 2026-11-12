//! **V&V, unit level (gh:#1366, epic #1336)** -- SAMM `bigeta` against compiled NJOY2016.
//!
//! ## Methodology
//!
//! `samm.f90`'s `bigeta` (A&S 14.6.7-8, eta >> rho) was extracted verbatim from NJOY2016
//! **2016.80** (`ab27c6414a0f25b173f2b637e6dc7e428fdaff00`) and **2016.79**
//! (`ac5adf5f33d893e42f2eed7fb286b0d51c7580da`), compiled with gfortran 13.3.0 `-O2`, and run on
//! 56 (eta, rho) pairs (every pair `coulx` sends to `bigeta`, grid in the V&V README) x lll in
//! {0,1,3} x flags {(0,0,-1),(1,1,1)} = 336 calls. The hex bit patterns of K0/K1 (and I0/I1) and of
//! p, s, dp, sinphi, cosphi, dphi are in
//! `verification_and_validation/upstream_units/samm_bigeta/upstream_output_v{79,80}.txt`
//! (regenerate with `run.sh`). The port's `samm::coulomb::bigeta` is run on the same inputs.
//! The port does not expose K0/K1, so a test-only mirror of the four series loops (same
//! operations as `dispatch.rs`) is run with three divisions: f64 (the port's), float32
//! (2016.80's `1./(k+1)`) and integer (2016.79's `1/(k+1)`), to attribute any difference.
//!
//! ## Prediction (written before running; V&V README)
//! Port vs 2016.80: K0/K1 about 1e-8 relative or less, not bit-identical except at tiny q;
//! port vs 2016.79: the whole harmonic term; I0, I1 bit-identical.
//!
//! ## Results (2026-11-10, rustc via workspace toolchain, release)
//! See the constants asserted below and the V&V README table; the bit-identity test against
//! 2016.80 is `#[ignore]`d until the sync (gh:#1368) changes the port's division.

use njoy_outram_park_fork::samm::coulomb::bigeta;
use petir::mathf::RealMath;

const V80: &str = include_str!(
    "../verification_and_validation/upstream_units/samm_bigeta/upstream_output_v80.txt"
);
const V79: &str = include_str!(
    "../verification_and_validation/upstream_units/samm_bigeta/upstream_output_v79.txt"
);

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

struct KLine {
    eta: f64,
    rho: f64,
    k: [f64; 4], // I0, K0, I1, K1
}

struct BLine {
    eta: f64,
    rho: f64,
    lll: i32,
    jdopha: bool,
    jdoder: bool,
    out: [f64; 6], // p s dp sinphi cosphi dphi
    fg: [f64; 4],  // F G F' G' at L=lll
}

fn parse(text: &str) -> (Vec<KLine>, Vec<BLine>) {
    let (mut ks, mut bs) = (vec![], vec![]);
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t[0] {
            "K" => ks.push(KLine {
                eta: hex(t[1]),
                rho: hex(t[2]),
                k: [hex(t[3]), hex(t[4]), hex(t[5]), hex(t[6])],
            }),
            "B" => bs.push(BLine {
                eta: hex(t[1]),
                rho: hex(t[2]),
                lll: t[3].parse().unwrap(),
                jdopha: t[4] == "1",
                jdoder: t[5] == "1",
                out: [
                    hex(t[6]),
                    hex(t[7]),
                    hex(t[8]),
                    hex(t[9]),
                    hex(t[10]),
                    hex(t[11]),
                ],
                fg: [hex(t[12]), hex(t[13]), hex(t[14]), hex(t[15])],
            }),
            "X" => {}
            _ => panic!("bad line {line}"),
        }
    }
    (ks, bs)
}

#[derive(Clone, Copy)]
enum Div {
    F64,
    F32,
    Int,
}

/// Test-only mirror of `bigeta`'s four series (dispatch.rs lines for I0, K0, I1, K1) with the
/// division in the harmonic sum `b` selectable. Returns (I0, K0, I1, K1).
fn kseries(eta: f64, rho: f64, div: Div) -> [f64; 4] {
    let h = |k: i32| -> f64 {
        match div {
            Div::F64 => 1.0 / (k + 1) as f64,
            Div::F32 => (1.0_f32 / (k + 1) as f32) as f64,
            Div::Int => (1 / (k + 1)) as f64,
        }
    };
    let euler = 0.577_215_664_901_532_86_f64;
    let q = 2.0 * rho * eta;
    let zhalf = q.sqrt();
    let z = zhalf * 2.0;
    let mut sum = 1.0_f64;
    let mut a = q;
    for k in 1..=100 {
        if sum + a == sum {
            break;
        }
        sum += a;
        a = a * q / ((k + 1) as f64).r_powi(2);
    }
    let ai0 = sum;
    let mut sum = -(zhalf.r_ln() + euler) * ai0;
    let mut a = q;
    let mut b = 1.0_f64;
    for k in 1..=100 {
        if sum + a * b == sum {
            break;
        }
        sum += a * b;
        b += h(k);
        a = a * q / ((k + 1) as f64).r_powi(2);
    }
    let ak0 = sum;
    let mut sum = 1.0_f64;
    let mut a = q / 2.0;
    for k in 1..=100 {
        if sum + a == sum {
            break;
        }
        sum += a;
        a = a * q / (((k + 1) * (k + 2)) as f64);
    }
    let ai1 = sum * zhalf;
    let mut sum = 1.0 / z + (zhalf.r_ln() + euler) * ai1 - zhalf * 0.5;
    let mut a = zhalf * q / 2.0;
    let mut b = 1.0_f64;
    let mut c = 0.25_f64;
    for k in 1..=100 {
        if sum - a * (b + c) == sum {
            break;
        }
        sum -= a * (b + c);
        c = 0.5 / (k + 2) as f64;
        b += h(k);
        a = a * q / (((k + 1) * (k + 2)) as f64);
    }
    [ai0, ak0, ai1, sum]
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// (bit-identical count, total, worst relative difference)
fn stats(pairs: &[(f64, f64)]) -> (usize, usize, f64) {
    let same = pairs.iter().filter(|(a, b)| a.to_bits() == b.to_bits()).count();
    let worst = pairs.iter().map(|&(a, b)| rel(a, b)).fold(0.0, f64::max);
    (same, pairs.len(), worst)
}

fn port_outputs(b: &BLine) -> [f64; 6] {
    let r = bigeta(b.eta, b.rho, b.lll, b.jdopha, b.jdoder, if b.jdoder { 1 } else { -1 })
        .expect("port bigeta converged");
    [r.p, r.s, r.dp, r.sinphi, r.cosphi, r.dphi]
}

/// Print-and-return the full table (used by the pinned tests).
fn measure() -> String {
    let (k80, b80) = parse(V80);
    let (k79, b79) = parse(V79);
    assert_eq!((k80.len(), b80.len()), (56, 336));
    assert_eq!((k79.len(), b79.len()), (56, 336));
    let mut s = String::new();
    // K series: mirror variants against each Fortran version.
    for (name, div) in [("f64", Div::F64), ("f32", Div::F32), ("int", Div::Int)] {
        for (ver, ks) in [("v80", &k80), ("v79", &k79)] {
            for (qi, qn) in ["I0", "K0", "I1", "K1"].iter().enumerate() {
                let pairs: Vec<(f64, f64)> = ks
                    .iter()
                    .map(|l| (kseries(l.eta, l.rho, div)[qi], l.k[qi]))
                    .collect();
                let (n, t, w) = stats(&pairs);
                s += &format!("mirror[{name}] vs {ver} {qn}: {n}/{t} bit-identical, worst {w:.3e}\n");
            }
        }
    }
    // Fortran v79 vs v80 themselves.
    for (qi, qn) in ["I0", "K0", "I1", "K1"].iter().enumerate() {
        let pairs: Vec<(f64, f64)> = k80.iter().zip(&k79).map(|(a, b)| (a.k[qi], b.k[qi])).collect();
        let (n, t, w) = stats(&pairs);
        s += &format!("fortran v80 vs v79 {qn}: {n}/{t}, worst {w:.3e}\n");
    }
    // Port bigeta against each version, per returned quantity.
    for (ver, bs) in [("v80", &b80), ("v79", &b79)] {
        for (qi, qn) in ["p", "s", "dp", "sinphi", "cosphi", "dphi"].iter().enumerate() {
            let pairs: Vec<(f64, f64)> = bs.iter().map(|b| (port_outputs(b)[qi], b.out[qi])).collect();
            let (n, t, w) = stats(&pairs);
            s += &format!("port vs {ver} {qn}: {n}/{t} bit-identical, worst {w:.3e}\n");
        }
    }
    for (qi, qn) in ["p", "s", "dp", "sinphi", "cosphi", "dphi"].iter().enumerate() {
        let pairs: Vec<(f64, f64)> = b80.iter().zip(&b79).map(|(a, b)| (a.out[qi], b.out[qi])).collect();
        let (n, t, w) = stats(&pairs);
        s += &format!("fortran v80 vs v79 {qn}: {n}/{t}, worst {w:.3e}\n");
    }
    for (qi, qn) in ["F", "G", "F'", "G'"].iter().enumerate() {
        let pairs: Vec<(f64, f64)> = b80.iter().zip(&b79).map(|(a, b)| (a.fg[qi], b.fg[qi])).collect();
        let (n, t, w) = stats(&pairs);
        s += &format!("fortran v80 vs v79 {qn}(L=lll): {n}/{t}, worst {w:.3e}\n");
    }
    s
}

/// The port's `bigeta` against NJOY2016 **2016.80**: all 336 calls, six returned quantities each,
/// compared as bit patterns. Pass criterion: every bit. Result after the 2016.80 sync of the
/// K0/K1 harmonic sums (gh:#1368; port change in `dispatch.rs::harmonic_step`):
/// 336/336 calls bit-identical in p, s, dp, sinphi, dphi; cosphi 316/336 over all calls and
/// 336/336 whenever the phase is requested (jdopha = 1; see the comment in the body). Two port defects were fixed: the f64 harmonic division (1) and `getfg`'s `llmax`
/// argument (2). BEFORE any change: p 12/336, s 175, dp 263, sinphi 173, cosphi 207, dphi 316
/// bit-identical, worst relative difference 1.0 to 1.9. After (1) alone: p 224, s 280, dp 280,
/// sinphi 254, cosphi 207, dphi 329 (the remainder all at lll >= 1, from (2)).
#[test]
fn bigeta_bit_identical_to_2016_80() {
    let (_, b80) = parse(V80);
    assert_eq!(b80.len(), 336);
    for b in &b80 {
        let got = port_outputs(b);
        for (qi, qn) in ["p", "s", "dp", "sinphi", "cosphi", "dphi"].iter().enumerate() {
            // Upstream's `getps` leaves cosphi untouched when jdopha = 0 (it keeps the caller's
            // pspcou initial value 1); the port's `getps` returns 0 there. cosphi is not a
            // requested output then, so it is compared only when the phase is requested.
            if qn == &"cosphi" && !b.jdopha {
                continue;
            }
            assert_eq!(
                got[qi].to_bits(),
                b.out[qi].to_bits(),
                "{qn} eta={} rho={} lll={} port={:e} v80={:e}",
                b.eta,
                b.rho,
                b.lll,
                got[qi],
                b.out[qi]
            );
        }
    }
}

/// Attribution of the 2016.79 -> 2016.80 change on K0 and K1 (the port exposes only P, S, phi,
/// so a test-only mirror of the series carries it). Measured 2026-11-10 over 56 (eta, rho):
/// the f32-division mirror reproduces 2016.80's I0, K0, I1, K1 in **56/56** bits; the integer
/// division mirror reproduces 2016.79's in 56/56; the f64-division mirror (the port's old line)
/// matches **neither**: K0 0/56 and K1 3/56 against 2016.80, 0/56 and 0/56 against 2016.79,
/// worst relative difference 1.0 (K0 is a cancelling sum, so the 6e-8 float32 rounding of
/// `1/(k+1)` is amplified to O(1) at large `2*rho*eta`; the issue's "~1e-8" prediction held
/// only at small q). I0 and I1 (no changed line) are 56/56 bit-identical in every variant.
#[test]
fn k_series_mirror_attributes_the_2016_80_change() {
    let (k80, _) = parse(V80);
    let (k79, _) = parse(V79);
    for l in &k80 {
        let m = kseries(l.eta, l.rho, Div::F32);
        assert!((0..4).all(|i| m[i].to_bits() == l.k[i].to_bits()), "f32 mirror vs v80");
        let m = kseries(l.eta, l.rho, Div::F64);
        assert_eq!(m[0].to_bits(), l.k[0].to_bits(), "I0 unaffected");
        assert_eq!(m[2].to_bits(), l.k[2].to_bits(), "I1 unaffected");
    }
    for l in &k79 {
        let m = kseries(l.eta, l.rho, Div::Int);
        assert!((0..4).all(|i| m[i].to_bits() == l.k[i].to_bits()), "int mirror vs v79");
    }
    let f64_k0: Vec<(f64, f64)> =
        k80.iter().map(|l| (kseries(l.eta, l.rho, Div::F64)[1], l.k[1])).collect();
    let (same, total, worst) = stats(&f64_k0);
    assert_eq!((same, total), (0, 56));
    assert!(worst > 0.5, "worst {worst}");
}

/// The CURRENT measured difference of the port against 2016.79 (the superseded baseline),
/// which equals 2016.80 against 2016.79 since the port now follows 2016.80: `p` is bit-identical
/// in **0/336** calls; Fortran K0 and K1 are identical in 0/56 each (the whole harmonic term is
/// missing in 2016.79), worst relative difference of K0 1.83, of K1 1.0.
#[test]
fn bigeta_differs_from_2016_79_by_the_harmonic_term() {
    let (_, b79) = parse(V79);
    let same_p = b79.iter().filter(|b| port_outputs(b)[0].to_bits() == b.out[0].to_bits()).count();
    assert_eq!(same_p, 0);
    let (k80, _) = parse(V80);
    let (k79, _) = parse(V79);
    for qi in [1usize, 3] {
        let pairs: Vec<(f64, f64)> =
            k80.iter().zip(&k79).map(|(a, b)| (a.k[qi], b.k[qi])).collect();
        let (n, t, _) = stats(&pairs);
        assert_eq!((n, t), (0, 56));
    }
}

/// The elementary functions `bigeta` calls after the series: `exp(-pi*eta)`, `sqrt(pi*rho)`,
/// `sqrt(2*pi*eta)`, port-side (`petir` `r_exp`, std `sqrt`) against gfortran/glibc.
#[test]
fn print_elementary_function_agreement() {
    let (mut e_same, mut n) = (0, 0);
    let mut worst = 0.0_f64;
    for line in V80.lines().filter(|l| l.starts_with("X ")) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let (eta, rho) = (hex(t[1]), hex(t[2]));
        let pi = std::f64::consts::PI;
        let e = (-pi * eta).r_exp();
        n += 1;
        if e.to_bits() == hex(t[3]).to_bits() {
            e_same += 1;
        }
        worst = worst.max(rel(e, hex(t[3])));
        assert_eq!((pi * rho).sqrt().to_bits(), hex(t[4]).to_bits());
        assert_eq!((2.0 * pi * eta).sqrt().to_bits(), hex(t[5]).to_bits());
    }
    println!("exp(-pi*eta) r_exp vs glibc: {e_same}/{n} bit-identical, worst {worst:e}");
}

#[test]
fn print_mismatch_pattern() {
    let (_, b80) = parse(V80);
    for b in b80.iter().filter(|b| b.lll == 0 && !b.jdopha) {
        let got = port_outputs(b);
        let ok = got[0].to_bits() == b.out[0].to_bits();
        println!("MM eta={} rho={} q={} p_same={} port={:e} up={:e}", b.eta, b.rho, 2.0 * b.eta * b.rho, ok, got[0], b.out[0]);
    }
}

/// Prints the full comparison table (V&V README source). Run with `--nocapture`.
#[test]
fn print_measurements() {
    println!("{}", measure());
}
