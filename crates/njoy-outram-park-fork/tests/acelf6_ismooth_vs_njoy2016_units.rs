//! **V&V, unit level (gh:#1368, #1366, epic #1336)** -- ACER `acelf6`'s `ismooth` sqrt(E)
//! low-energy extensions (LAW=1, LANG=2, LEP=1 histogram and LEP=2 lin-lin) against compiled
//! NJOY2016 **2016.80** and **2016.79**.
//!
//! ## Methodology
//!
//! The two `ismooth` branches were extracted verbatim from `acelf6` in `acefc.f90` of NJOY2016
//! **2016.80** (`ab27c6414a0f25b173f2b637e6dc7e428fdaff00`) and **2016.79**
//! (`ac5adf5f33d893e42f2eed7fb286b0d51c7580da`) by
//! `verification_and_validation/upstream_units/acelf6_ismooth/extract.sh` (SHA-256 checked),
//! compiled with gfortran 13.3.0 `-O2`, and run by `driver.f90` on 12 LAW=1/LANG=2 incident-energy
//! records (LEP=1 and 2, NA=0 and 1, ND=0 and 1, first outgoing energy E'_1 = 0, 1e-3 eV, 30 eV,
//! 500 eV, 1 keV). The inputs and the resulting record (`scr(7..6+nx)`, `n`) are hex bit
//! patterns in `upstream_output_v{80,79}.txt`; this test reads the inputs from there, so
//! the case table lives in one place. The port's
//! `acer::acelf6::ismooth_extend_low_energy` (the block lifted out of `incident_block` unchanged,
//! plus the 2016.80 guard) runs on the same records.
//!
//! ## What upstream changed (`7d317f8`, 2016.79 -> 2016.80)
//! Both branches additionally require `eout1 = scr(7)/emev` to be zero, i.e. the first
//! tabulated outgoing energy is exactly 0.
//!
//! ## Prediction (written before the port change)
//! The port (which had no guard) differs from 2016.80 exactly on the records whose E'_1 > 0 and
//! which the unguarded branch extends (cases 2, 3, 6, 7, 8, 10: 2016.79 changes them, 2016.80
//! leaves them as read) and is identical on the others (1, 4, 5, 9, 11, 12). Against 2016.79 the
//! port was bit-identical in all 12. After the change: bit-identical to 2016.80 in all 12, and
//! the outgoing-energy ordering of every 2016.80 output is monotonic (while 2016.79's is not in
//! at least one E'_1 > 0 record).
//!
//! ## Results (2026-11-10, gfortran 13.3.0 `-O2`, rustc 1.99.1 release)
//! After the guard: port vs 2016.80 **12/12** bit-identical (row count and every word); port vs
//! 2016.79 identical in exactly the 6 records the guard does not touch. 2016.80 vs 2016.79 differ in
//! the 6 records predicted. Prediction held on every count; the one detail it missed is that
//! only case 2 of the six produces out-of-order energies. **Before the guard** (measured by
//! temporarily switching the guard off, same reference): the port equalled 2016.79 in **12/12**
//! records (so its `ismooth` port was a faithful 2016.79 translation) and therefore 2016.80 in
//! only 6/12, failing first at case 2 (port 34 rows, 2016.80 6 rows).
//! (Harness note: the first reference had its `E` line cut at 60 words by a Fortran format, which
//! hid case 2's 68-word output; fixed in `driver.f90` and regenerated before these results.)

use njoy_outram_park_fork::acer::acelf6::ismooth_extend_low_energy;

const V80: &str = include_str!(
    "../verification_and_validation/upstream_units/acelf6_ismooth/upstream_output_v80.txt"
);
const V79: &str = include_str!(
    "../verification_and_validation/upstream_units/acelf6_ismooth/upstream_output_v79.txt"
);

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

struct Case {
    id: usize,
    lep: i32,
    nd: usize,
    na: usize,
    n: usize,
    input: Vec<f64>,
    out_n: usize,
    output: Vec<f64>,
}

fn parse(text: &str) -> Vec<Case> {
    let mut cases = vec![];
    let mut cur: Option<Case> = None;
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t[0] {
            "I" => {
                cur = Some(Case {
                    id: t[1].parse().unwrap(),
                    lep: t[2].parse().unwrap(),
                    nd: t[3].parse().unwrap(),
                    na: t[4].parse().unwrap(),
                    n: t[5].parse().unwrap(),
                    input: vec![],
                    out_n: 0,
                    output: vec![],
                });
            }
            "D" => cur.as_mut().unwrap().input = t[1..].iter().map(|h| hex(h)).collect(),
            "R" => cur.as_mut().unwrap().out_n = t[3].parse().unwrap(),
            "E" => {
                let mut c = cur.take().unwrap();
                c.output = t[1..].iter().map(|h| hex(h)).collect();
                cases.push(c);
            }
            _ => panic!("bad line {line}"),
        }
    }
    cases
}

fn run_port(c: &Case) -> (usize, Vec<f64>) {
    let mut d = c.input.clone();
    let ncyc = c.na + 2;
    let n = ismooth_extend_low_energy(&mut d, c.n, ncyc, c.nd, 2, c.lep);
    (n, d)
}

fn same(c: &Case, n: usize, d: &[f64]) -> bool {
    n == c.out_n
        && d.len() == c.output.len()
        && d.iter().zip(&c.output).all(|(a, b)| a.to_bits() == b.to_bits())
}

fn energies_monotonic(d: &[f64], ncyc: usize) -> bool {
    let e: Vec<f64> = d.chunks(ncyc).map(|r| r[0]).collect();
    e.windows(2).all(|w| w[0] <= w[1])
}

/// The port against 2016.80 on all 12 records: **12/12 bit-identical** (row count and every
/// word of the record) after the guard was added. BEFORE the guard the same comparison matched
/// 6/12: it differed on cases 2, 3, 6, 7, 8 and 10 (the six records with E'_1 > 0 that the
/// unguarded branch extends and 2016.80 leaves as read), as predicted. Pass criterion: every bit.
#[test]
fn ismooth_bit_identical_to_2016_80() {
    let cases = parse(V80);
    assert_eq!(cases.len(), 12);
    for c in &cases {
        let (n, d) = run_port(c);
        assert!(same(c, n, &d), "case {} differs from 2016.80 (port n={n}, v80 n={})", c.id, c.out_n);
    }
}

/// 2016.80 against 2016.79 differ on exactly the six records with E'_1 > 0 that the unguarded
/// branch extends (2, 3, 6, 7, 8, 10) and agree on the other six (1, 4, 5, 9, 11, 12). Measured:
/// of the six, only 2016.79's case 2 (E'_1 = 1 keV, LEP=1) has out-of-order outgoing energies
/// (the symptom `7d317f8` describes); cases 3, 6, 7, 8, 10 are extended without a disorder but
/// with spurious rows/changed distributions below E'_1. Every 2016.80 output is ordered.
#[test]
fn guard_changes_exactly_the_records_that_start_above_zero() {
    let c80 = parse(V80);
    let c79 = parse(V79);
    assert_eq!(c79.len(), 12);
    let differing: Vec<usize> = c80
        .iter()
        .zip(&c79)
        .filter(|(a, b)| !(a.out_n == b.out_n && a.output.iter().zip(&b.output).all(|(x, y)| x.to_bits() == y.to_bits())))
        .map(|(a, _)| a.id)
        .collect();
    assert_eq!(differing, vec![2, 3, 6, 7, 8, 10]);
    for (a, b) in c80.iter().zip(&c79) {
        let ncyc = a.na + 2;
        assert!(energies_monotonic(&a.output, ncyc), "v80 case {} energies out of order", a.id);
        if differing.contains(&a.id) {
            // Not every extended record is out of order (a first E' below the inserted energy
            // keeps order); record which ones are for the README.
            println!("case {}: v79 monotonic = {}", b.id, energies_monotonic(&b.output, ncyc));
        }
    }
    // At least the 1 keV case (2) shows the defect: its inserted rows precede nothing but follow
    // a first energy larger than them.
    let case2 = c79.iter().find(|c| c.id == 2).unwrap();
    assert!(!energies_monotonic(&case2.output, 2), "v79 case 2 should be out of order");
}

/// The port reproduces 2016.79 only on the records the guard does not touch (6/12 after the
/// change); this pins that the departure from 2016.79 is exactly the 2016.80 guard.
#[test]
fn port_matches_2016_79_only_where_the_guard_is_inert() {
    let cases = parse(V79);
    let matching: Vec<usize> = cases
        .iter()
        .filter(|c| {
            let (n, d) = run_port(c);
            same(c, n, &d)
        })
        .map(|c| c.id)
        .collect();
    assert_eq!(matching, vec![1, 4, 5, 9, 11, 12]);
}
