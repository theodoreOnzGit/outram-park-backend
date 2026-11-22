//! **V&V, unit level (gh:#1368, #1366, epic #1336)** -- SAMM `coulx`, the finite-eta branches
//! (everything `coulx` does not send to `bigeta`), against compiled NJOY2016 **2016.80**.
//!
//! ## Methodology
//!
//! `samm.f90`'s `coulx`, `asymp1`, `asymp2`, `taylor`, `xsigll`, `end1`, `getfg` and `getps`
//! were extracted verbatim from NJOY2016 2016.80 (`ab27c6414a0f25b173f2b637e6dc7e428fdaff00`,
//! SHA-256 of `samm.f90` checked by `extract.sh`) and compiled with gfortran 13.3.0 `-O2`
//! (no `-march`, so no FMA). `driver_coulx.f90` calls `coulx(eta, rho, lll, llmax = lll+2, ..)`
//! exactly as `pspcou` does (`samm.f90:3935`) on 11 eta x 10 rho (minus the pairs that go to
//! `bigeta`: 88 pairs) x lll {0,1,3} x flags {(0,0,-1),(1,1,1)} = 528 calls,
//! and prints p, s, dp, sinphi, cosphi, dphi as hex bits
//! (`verification_and_validation/upstream_units/samm_bigeta/upstream_output_coulx_v80.txt`).
//! The port's `samm::coulomb::coulx` is run on the same inputs.
//!
//! ## Prediction (written before the port change)
//!
//! Upstream passes `llmax = lll+2` to `getfg` in `coulx`; the port passed `lll`. `getfg`'s
//! `limit = max(3, llmax+1)` decides where the backward `F` recursion starts, so for `lll >= 1`
//! the port should differ from upstream in the last bits of p/s/dp/phase on the `getfg` path
//! (not on the `end1` path, and not at `lll = 0` where `limit = 3` either way). After the
//! change (`dispatch.rs::coulx` passes `lll + 2`): bit-identical in p, s, dp, sinphi, dphi.
//!
//! ## Results
//!
//! Before the change (`getfg(.., lll, lll, ..)`), bit-identical of 528: p 444, s 486, dp 486,
//! sinphi 419, dphi 425 (cosphi 222 of 264), worst relative difference 6.8e-4 (s). After
//! (`lll + 2`): **528/528** in p, s, dp, sinphi, dphi and 264/264 in cosphi. 2026-11-10,
//! gfortran 13.3.0 `-O2`, rustc 1.99.1 release. V&V README:
//! `verification_and_validation/upstream_units/samm_bigeta/README.md`.

use njoy_outram_park_fork::samm::coulomb::coulx;

const V80: &str = include_str!(
    "../verification_and_validation/upstream_units/samm_bigeta/upstream_output_coulx_v80.txt"
);

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

struct CLine {
    eta: f64,
    rho: f64,
    lll: i32,
    jdopha: bool,
    jdoder: bool,
    out: [f64; 6], // p s dp sinphi cosphi dphi
}

fn parse() -> Vec<CLine> {
    V80.lines()
        .filter(|l| l.starts_with("C "))
        .map(|line| {
            let t: Vec<&str> = line.split_whitespace().collect();
            CLine {
                eta: hex(t[1]),
                rho: hex(t[2]),
                lll: t[3].parse().unwrap(),
                jdopha: t[4] == "1",
                jdoder: t[5] == "1",
                out: [hex(t[6]), hex(t[7]), hex(t[8]), hex(t[9]), hex(t[10]), hex(t[11])],
            }
        })
        .collect()
}

const NAMES: [&str; 6] = ["p", "s", "dp", "sinphi", "cosphi", "dphi"];

fn port(c: &CLine) -> Option<[f64; 6]> {
    coulx(c.eta, c.rho, c.lll, c.jdopha, c.jdoder, if c.jdoder { 1 } else { -1 })
        .map(|r| [r.p, r.s, r.dp, r.sinphi, r.cosphi, r.dphi])
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// Counts per quantity: (bit-identical, total, worst relative difference).
fn measure() -> (Vec<(usize, usize, f64)>, usize) {
    let lines = parse();
    let mut stats = vec![(0usize, 0usize, 0.0f64); 6];
    let mut none = 0;
    for c in &lines {
        let Some(got) = port(c) else {
            none += 1;
            continue;
        };
        for qi in 0..6 {
            // Upstream's `getps` leaves cosphi at the caller's initial 1 when jdopha = 0 (not a
            // requested output); see samm_bigeta_vs_njoy2016_units.rs.
            if qi == 4 && !c.jdopha {
                continue;
            }
            let s = &mut stats[qi];
            s.1 += 1;
            if got[qi].to_bits() == c.out[qi].to_bits() {
                s.0 += 1;
            }
            s.2 = s.2.max(rel(got[qi], c.out[qi]));
        }
    }
    (stats, none)
}

/// The port's `coulx` against NJOY2016 **2016.80** on every finite-eta call (`asymp1`/`asymp2`/
/// `taylor`/`xsigll`/`end1`/`getfg`/`getps`), compared as bit patterns. Pass criterion: every
/// bit of p, s, dp, sinphi, dphi (cosphi when the phase is requested). Measured counts are in
/// the V&V README (`verification_and_validation/upstream_units/samm_bigeta/README.md`).
#[test]
fn coulx_bit_identical_to_2016_80() {
    let lines = parse();
    assert!(lines.len() > 300, "reference has {} calls", lines.len());
    for c in &lines {
        let got = port(c).expect("port coulx returned a value");
        for qi in 0..6 {
            if qi == 4 && !c.jdopha {
                continue;
            }
            assert_eq!(
                got[qi].to_bits(),
                c.out[qi].to_bits(),
                "{} eta={} rho={} lll={} jdopha={} jdoder={} port={:e} v80={:e}",
                NAMES[qi],
                c.eta,
                c.rho,
                c.lll,
                c.jdopha,
                c.jdoder,
                got[qi],
                c.out[qi]
            );
        }
    }
}

/// Prints the full comparison table (V&V README source). Run with `--nocapture`.
#[test]
fn print_coulx_measurements() {
    let (stats, none) = measure();
    println!("calls where the port returned None: {none}");
    for (qi, (same, total, worst)) in stats.iter().enumerate() {
        println!("coulx port vs v80 {}: {same}/{total} bit-identical, worst {worst:.3e}", NAMES[qi]);
    }
}
