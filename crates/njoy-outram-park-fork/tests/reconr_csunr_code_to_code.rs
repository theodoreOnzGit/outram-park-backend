// SPDX-License-Identifier: AGPL-3.0-only
//! **RECONR `csunr1`, `csunr2`, `unfac`, `gnrl` against the compiled NJOY2016.80
//! routines, bit for bit** (gh:#1448, differential #1366).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_units/reconr_csunr/` extracts the four
//! routines verbatim from `reconr.f90` (SHA-256 checked, lines 3826-4317 and
//! 4473-4644 of NJOY2016.80, commit `ab27c6414a`), links them in a module that
//! holds `reconr`'s state variables, fills the flat `res` array in the layout
//! `rdf2u0/rdf2u1/rdf2u2` store, and prints every result as hex bits
//! (gfortran 13.3.0 `-O2`). `cases.in` is the single input: five materials
//! (Case A with `NLS = 4` and so the `unfac` fall-through, a Case A with
//! `NAPS = 1`, Case B `LFW = 1` with panels both narrower and wider than
//! `wide = 1.26` and energies landing exactly on tabulated fission-width
//! energies, two Case C with `INT = 2`/`5` mixes and uneven J-state grids),
//! each evaluated at an ascending list of energies as `genunr` calls them,
//! with panel state carried from call to call. `gnrl` is called directly on
//! every branch (all `id`, all degrees of freedom 1-4, the four `gbeta`/`df`
//! regimes and the early-return guards) and `unfac` for `l = 0..3` on sentinel
//! `vl`/`ps` (so `l = 3` shows upstream's no-assignment).
//!
//! **Not covered:** `NRO = 1` (energy-dependent `AP(E)`, `terpa`), which is
//! the already-ported [`UnresolvedRange::scattering_radius`] and is exercised
//! at deck level (U-238/Pu-241 are `NRO = 0`; no `NRO = 1` URR tape here).
//!
//! # Prediction (written before the first run)
//!
//! Every value is **bit-identical**: the Rust is a statement-for-statement
//! translation with the same operation order, the arithmetic is IEEE
//! `+ - * /` and `sqrt`; the only libm calls are `sin` and `atan`, taken from
//! PETIR's `RealMath`, which agrees with glibc on this platform (the ACE
//! parity gates already depend on that). `gnrl` returns exact zeros on its
//! guards.
//!
//! # Result
//!
//! See the test docs below.

use njoy_outram_park_fork::reconr::csunr::{gnrl, unfac, UnrState};
use njoy_outram_park_fork::unresr::mf2::{
    JStateA, JStateB, JStateC, LState, UnresolvedCase, UnresolvedPointC, UnresolvedRange,
};

const CASES: &str = include_str!("../verification_and_validation/upstream_units/reconr_csunr/cases.in");
const UPSTREAM: &str =
    include_str!("../verification_and_validation/upstream_units/reconr_csunr/upstream_output.txt");

struct Tokens<'a>(std::str::SplitWhitespace<'a>);

impl Tokens<'_> {
    fn s(&mut self) -> String {
        self.0.next().expect("token").to_string()
    }
    fn f(&mut self) -> f64 {
        self.s().parse().expect("float")
    }
    fn i(&mut self) -> i32 {
        self.s().parse().expect("int")
    }
}

/// One material of `cases.in` and its ascending call energies.
fn read_cases() -> Vec<(UnresolvedRange, Vec<f64>)> {
    let mut t = Tokens(CASES.split_whitespace());
    let ncase = t.i();
    let mut out = Vec::new();
    for _ in 0..ncase {
        let kind = t.s();
        let naps = t.i();
        let ap = t.f();
        let spi = t.f();
        let awri = t.f();
        let nls = t.i();
        let ne = t.i() as usize;
        let fe: Vec<f64> = (0..ne).map(|_| t.f()).collect();
        let mut la: Vec<LState<JStateA>> = Vec::new();
        let mut lb: Vec<LState<JStateB>> = Vec::new();
        let mut lc: Vec<LState<JStateC>> = Vec::new();
        for _ in 0..nls {
            let l = t.i();
            let njs = t.i();
            let (mut ja, mut jb, mut jc) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..njs {
                match kind.as_str() {
                    "A" => ja.push(JStateA { d: t.f(), aj: t.f(), amun: t.f(), gno: t.f(), gg: t.f() }),
                    "B" => {
                        let amuf = t.f();
                        let (d, aj, amun, gno, gg) = (t.f(), t.f(), t.f(), t.f(), t.f());
                        let gf = (0..ne).map(|_| t.f()).collect();
                        jb.push(JStateB { amuf, d, aj, amun, gno, gg, gf });
                    }
                    _ => {
                        let (aj, int_, amux, amun, amuf) = (t.f(), t.i(), t.f(), t.f(), t.f());
                        let npts = t.i();
                        let points = (0..npts)
                            .map(|_| UnresolvedPointC { e: t.f(), d: t.f(), gx: t.f(), gno: t.f(), gg: t.f(), gf: t.f() })
                            .collect();
                        jc.push(JStateC { aj, amux, amun, amuf, int_, points });
                    }
                }
            }
            match kind.as_str() {
                "A" => la.push(LState { l, j_states: ja }),
                "B" => lb.push(LState { l, j_states: jb }),
                _ => lc.push(LState { l, j_states: jc }),
            }
        }
        let case_ = match kind.as_str() {
            "A" => UnresolvedCase::CaseA { awri, ap, spi, l_states: la },
            "B" => UnresolvedCase::CaseB { awri, ap, spi, fission_energies: fe, l_states: lb },
            _ => UnresolvedCase::CaseC { awri, ap, spi, l_states: lc },
        };
        let nener = t.i() as usize;
        let en: Vec<f64> = (0..nener).map(|_| t.f()).collect();
        let range =
            UnresolvedRange { el: 1.0e-5, eh: 1.0e7, abn: 1.0, naps, nro: 0, ap_table: None, lssf: 0, case_ };
        out.push((range, en));
    }
    out
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).expect("hex")
}

/// **`csunr1`/`csunr2` (with `unfac`, `gnrl` inside), 88 calls over 5 materials.**
///
/// Pass criterion: every returned cross section of every call is within 4e-16
/// relative of the extracted NJOY2016.80 routines, panel state carried across
/// the calls of one material, and the count of bit-identical values is the
/// recorded one.
///
/// **Result (2026-11-11, rustc per rust-toolchain.toml, gfortran 13.3.0 -O2,
/// glibc 2.39): 342 of 352 values bit-identical; the other 10 (the total and
/// the elastic cross section of the five Case B calls at 1.5e4, 3e4, 4.1e4,
/// 4.2e4 and 4.4 e4 eV) differ by one unit in the last place, worst 3.1e-16
/// relative.** (The prediction, all bit-identical, was wrong in this respect.)
/// Cause, shown rather than assumed: those are the values that carry
/// `sin(ps)` (potential scattering and the interference term); with `ps.sin()`
/// from Rust's `std` (the system libm, glibc 2.39, which gfortran calls)
/// instead of PETIR's `RealMath::r_sin` the result is **352 of 352
/// bit-identical**. So PETIR's `r_sin` differs from glibc's `sin` by one ulp at
/// some arguments. This crate's rule is that every transcendental goes through
/// `RealMath` (gh:#985), so the port keeps `r_sin`; the allowance for round-off
/// below 1e-8 relative with an understood cause applies, and the
/// difference is flagged for the maintainer (it is not a defect of this
/// translation). A pin on the count would catch either route changing.
#[test]
fn csunr1_and_csunr2_are_bit_identical_to_njoy2016() {
    let cases = read_cases();
    let mut rows = UPSTREAM.lines().filter(|l| l.starts_with("S "));
    let (mut calls, mut same, mut values) = (0, 0, 0);
    let mut first_bad: Option<(usize, usize)> = None;
    let mut worst = 0.0_f64;
    for (ic, (range, energies)) in cases.iter().enumerate() {
        let mut st = UnrState::new(1);
        for (i, &e) in energies.iter().enumerate() {
            let row: Vec<&str> = rows.next().expect("upstream row").split_whitespace().collect();
            assert_eq!(row[1].parse::<usize>().unwrap(), ic + 1);
            assert_eq!(row[2].parse::<usize>().unwrap(), i + 1);
            assert_eq!(hex(row[3]), e.to_bits(), "energy bits");
            let got = st.evaluate(0, range, e).expect("evaluate");
            calls += 1;
            let mut ok = true;
            for k in 0..4 {
                values += 1;
                if got[k].to_bits() == hex(row[4 + k]) {
                    same += 1;
                } else {
                    ok = false;
                }
            }
            if !ok {
                let theirs: Vec<f64> = row[4..8].iter().map(|h| f64::from_bits(hex(h))).collect();
                for k in 0..4 {
                    worst = f64::max(worst, ((got[k] - theirs[k]) / theirs[k]).abs());
                }
                println!("DIFF case {} call {} E={e} ours={got:?} njoy={theirs:?}", ic + 1, i + 1);
                first_bad.get_or_insert((ic + 1, i + 1));
            }
        }
    }
    println!("csunr: {calls} calls, {same}/{values} values bit-identical");
    println!("worst relative difference {worst:e}, first differing call {first_bad:?}");
    assert_eq!(calls, 88);
    // 342 of 352 bit-identical; the other 10 values (elastic and total of five
    // Case B calls) differ by one ulp, from `sin`: see the doc above.
    assert_eq!(same, 342, "recorded 342/352; first differing call {first_bad:?}");
    assert!(worst < 4.0e-16, "worst relative difference {worst:e}");
}

/// **`gnrl`: every branch** — 1 152 calls (`gbeta`/`df` regimes x `mu`, `nu`,
/// `lamda` in 1..4 x `id` 1..3) and the three early-return guards, bit-identical
/// to NJOY2016.80 (`id = 3` with `gbeta = 0` returns upstream's zero).
#[test]
fn gnrl_is_bit_identical_on_every_branch() {
    let gb = [0.0, 0.0, 0.3, 0.3, -0.1, 0.3];
    let df = [0.0, 0.2, 0.0, 0.2, 0.0, -0.2];
    let mut n = 0;
    for line in UPSTREAM.lines().filter(|l| l.starts_with("G ")) {
        let p: Vec<&str> = line.split_whitespace().collect();
        let (ib, mu, nu, lam, id): (usize, i32, i32, i32, i32) =
            (p[1].parse().unwrap(), p[2].parse().unwrap(), p[3].parse().unwrap(), p[4].parse().unwrap(), p[5].parse().unwrap());
        let got = gnrl(0.05, gb[ib - 1], 0.04, mu, nu, lam, df[ib - 1], id).unwrap();
        assert_eq!(got.to_bits(), hex(p[6]), "gnrl ib={ib} mu={mu} nu={nu} lam={lam} id={id}");
        n += 1;
    }
    assert_eq!(n, 1152);
    assert_eq!(gnrl(0.0, 0.3, 0.04, 2, 2, 2, 0.2, 1).unwrap(), 0.0);
    assert_eq!(gnrl(0.05, 0.3, 0.0, 2, 2, 2, 0.2, 1).unwrap(), 0.0);
    assert_eq!(gnrl(-0.05, 0.3, 0.04, 2, 2, 2, 0.2, 2).unwrap(), 0.0);
    // upstream indexes `qw(j,0)`; the port refuses
    assert!(gnrl(0.05, 0.0, 0.04, 0, 1, 1, 0.0, 1).is_err());
}

/// **`unfac`: `l = 0..3`, bit-identical**, and for `l = 3` the sentinel
/// `vl = 7`, `ps = 8` come back unchanged (upstream has no `else`).
#[test]
fn unfac_is_bit_identical_and_leaves_l3_alone() {
    let rho = [0.05, 0.8, 2.5];
    let rhoc = [0.06, 0.9, 2.2];
    let mut n = 0;
    for line in UPSTREAM.lines().filter(|l| l.starts_with("U ")) {
        let p: Vec<&str> = line.split_whitespace().collect();
        let (l, k): (i32, usize) = (p[1].parse().unwrap(), p[2].parse().unwrap());
        let (mut vl, mut ps) = (7.0, 8.0);
        unfac(l, rho[k - 1], rhoc[k - 1], 2.0, &mut vl, &mut ps);
        assert_eq!((vl.to_bits(), ps.to_bits()), (hex(p[3]), hex(p[4])), "unfac l={l} k={k}");
        if l == 3 {
            assert_eq!((vl, ps), (7.0, 8.0));
        }
        n += 1;
    }
    assert_eq!(n, 12);
}
