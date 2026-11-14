// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::{QawoTable, qawo, qawf}` against compiled GSL
//! 2.8** (gh:#1340, round 3). Methodology, reference and predictions P11 and
//! P12: `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/qawo-gsl-2.8-reference.txt` (driver
//! `qawo_reference_driver.c`): 12 moment-table rows (including a
//! `set_length` / `set` sequence), 28 QAWO integrations and 18 QAWF
//! integrations. GSL's integer status, workspace size(s) and the number of
//! integrand evaluations must match **exactly**; Chebyshev moments, results
//! and error estimates are counted bit-identical and the counts pinned. The
//! weights call `sin` and `cos`, which are glibc in GSL and PETIR's
//! (`libm` crate) in the port; the integrands call none.

use petir::integration::{qawf_with_status, qawo_with_status, QawoKind, QawoTable};
use std::cell::Cell;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/qawo-gsl-2.8-reference.txt");

type Fx = fn(f64) -> f64;

fn rec1(x: f64) -> f64 {
    1.0 / x
}

fn noise(x: f64) -> f64 {
    let y = x * 1.0e6;
    1.0e-3 * (y - y.floor())
}

fn integrand(name: &str) -> Fx {
    match name {
        "one" => |_| 1.0,
        "zero" => |_| 0.0,
        "lin" => |x| x,
        "expmx" => |x| (-x).exp(),
        "expx" => |x| x.exp(),
        "invsqrt" => |x| 1.0 / x.sqrt(),
        "logx" => |x| x.ln(),
        "lorentz" => |x| 1.0 / (1.0 + x * x),
        "rec1p" => |x| 1.0 / (1.0 + x),
        "rec1" => rec1,
        "expm2x" => |x| (-(x + 2.0)).exp(),
        "gauss" => |x| (-x * x).exp(),
        "noise" => noise,
        other => panic!("unknown integrand {other}"),
    }
}

fn kind(sine: bool) -> QawoKind {
    if sine {
        QawoKind::Sine
    } else {
        QawoKind::Cosine
    }
}

/// `(integrand, a, omega, L, sine, levels, epsabs, epsrel, limit)`.
type OCase = (&'static str, f64, f64, f64, bool, usize, f64, f64, usize);

fn qawo_case(name: &str) -> OCase {
    const TAU: f64 = 6.283185307179586;
    match name {
        "exp_cos10" => ("expmx", 0.0, 10.0, 1.0, false, 25, 0.0, 1e-10, 100),
        "x_sin40" => ("lin", 0.0, 40.0, 1.0, true, 25, 0.0, 1e-10, 100),
        "one_cos_low" => ("one", 0.0, 0.5, 1.0, false, 10, 0.0, 1e-10, 100),
        "one_sin_low" => ("one", 0.0, 1.5, 1.0, true, 10, 0.0, 1e-10, 100),
        "invsqrt_sin20" => ("invsqrt", 0.0, 20.0, 1.0, true, 25, 0.0, 1e-8, 200),
        "invsqrt_cos100" => ("invsqrt", 0.0, 100.0, 2.0, false, 30, 0.0, 1e-8, 300),
        "log_cos100" => ("logx", 0.0, 100.0, TAU, false, 30, 0.0, 1e-8, 300),
        "expx_sin1000" => ("expx", 0.0, 1000.0, 1.0, true, 25, 0.0, 1e-10, 100),
        "big_par_cos" => ("expmx", 0.0, 100.0, 1.0, false, 25, 0.0, 1e-10, 100),
        "big_par_sin" => ("expmx", 0.0, 100.0, 1.0, true, 25, 0.0, 1e-10, 100),
        "neg_omega" => ("rec1p", 0.0, -15.0, 2.0, true, 25, 0.0, 1e-10, 100),
        "a_negative" => ("lorentz", -2.0, 7.0, 4.0, false, 25, 0.0, 1e-10, 100),
        "gauss_cos3" => ("gauss", -3.0, 3.0, 6.0, false, 25, 1e-12, 0.0, 100),
        "omega_zero_cos" => ("expmx", 0.0, 0.0, 1.0, false, 5, 0.0, 1e-10, 100),
        "omega_zero_sin" => ("expmx", 0.0, 0.0, 1.0, true, 5, 0.0, 1e-10, 100),
        "zero_f" => ("zero", 0.0, 30.0, 1.0, false, 5, 0.0, 1e-10, 100),
        "noise_sin" => ("noise", 0.0, 50.0, 1.0, true, 25, 0.0, 1e-12, 200),
        "hard_extrap" => ("invsqrt", 0.0, 400.0, 1.0, false, 40, 0.0, 1e-13, 500),
        "limit1" => ("invsqrt", 0.0, 20.0, 1.0, true, 25, 0.0, 1e-12, 1),
        "limit2" => ("invsqrt", 0.0, 20.0, 1.0, true, 25, 0.0, 1e-12, 2),
        "limit4" => ("invsqrt", 0.0, 20.0, 1.0, true, 25, 0.0, 1e-12, 4),
        "limit10" => ("invsqrt", 0.0, 20.0, 1.0, true, 25, 0.0, 1e-12, 10),
        "table_n1" => ("invsqrt", 0.0, 200.0, 1.0, false, 1, 0.0, 1e-12, 100),
        "table_n3" => ("invsqrt", 0.0, 200.0, 1.0, false, 3, 0.0, 1e-12, 100),
        "table_n6" => ("invsqrt", 0.0, 200.0, 1.0, false, 6, 0.0, 1e-12, 100),
        "badtol" => ("one", 0.0, 10.0, 1.0, false, 5, 0.0, 1e-17, 10),
        "abs_only" => ("expmx", 0.0, 12.0, 3.0, true, 25, 1e-9, 0.0, 100),
        "long_interval" => ("rec1p", 0.0, 3.0, 40.0, false, 30, 0.0, 1e-10, 300),
        other => panic!("unknown qawo case {other}"),
    }
}

/// `(integrand, a, omega, sine, epsabs, limit, cycle_limit)`.
type FCase = (&'static str, f64, f64, bool, f64, usize, usize);

fn qawf_case(name: &str) -> FCase {
    match name {
        "expmx_cos1" => ("expmx", 0.0, 1.0, false, 1e-10, 50, 50),
        "lorentz_cos2" => ("lorentz", 0.0, 2.0, false, 1e-10, 50, 50),
        "rec1_sin1" => ("rec1", 1.0, 1.0, true, 1e-8, 50, 50),
        "expmx_sin5" => ("expmx", 0.0, 5.0, true, 1e-10, 50, 50),
        "omega0_cos" => ("expmx", 0.0, 0.0, false, 1e-10, 10, 100),
        "omega0_sin" => ("expmx", 0.0, 0.0, true, 1e-10, 10, 100),
        "badtol" => ("expmx", 0.0, 1.0, false, 0.0, 10, 10),
        "limit3" => ("rec1p", 0.0, 10.0, true, 1e-10, 3, 20),
        "limit8" => ("rec1p", 0.0, 10.0, true, 1e-10, 8, 20),
        "cycle_lt_limit" => ("expmx", 0.0, 1.0, false, 1e-10, 20, 10),
        "neg_omega" => ("expmx", 0.0, -3.0, true, 1e-10, 50, 50),
        "invsqrt_cos3" => ("invsqrt", 0.0, 3.0, false, 1e-8, 50, 100),
        "rec1p_sin10" => ("rec1p", 0.0, 10.0, true, 1e-8, 100, 100),
        "a_offset" => ("expm2x", -1.5, 2.0, false, 1e-10, 50, 50),
        "omega50" => ("expmx", 0.0, 50.0, false, 1e-10, 50, 50),
        "tight" => ("expmx", 0.0, 1.0, true, 1e-14, 80, 80),
        "const_cos" => ("one", 0.0, 1.0, false, 1e-8, 30, 30),
        "tiny_eps" => ("expmx", 0.0, 2.0, false, 1e-300, 60, 60),
        other => panic!("unknown qawf case {other}"),
    }
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
    }
}

#[derive(Default)]
struct Tally {
    rows: usize,
    status: usize,
    size: usize,
    neval: usize,
    result_exact: usize,
    abserr_exact: usize,
    worst_result: f64,
    worst_abserr_ratio: f64,
}

impl Tally {
    fn note(&mut self, name: &str, got: (f64, f64), want: (f64, f64)) {
        if got.0.to_bits() == want.0.to_bits() || (got.0.is_nan() && want.0.is_nan()) {
            self.result_exact += 1;
        } else {
            let r = rel(got.0, want.0);
            println!("  {name}: result differs, rel {r:.2e}");
            self.worst_result = self.worst_result.max(r);
        }
        if got.1.to_bits() == want.1.to_bits() || (got.1.is_nan() && want.1.is_nan()) {
            self.abserr_exact += 1;
        } else {
            let ratio = (got.1 / want.1).max(want.1 / got.1);
            println!("  {name}: abserr differs, ratio {ratio:.6}");
            self.worst_abserr_ratio = self.worst_abserr_ratio.max(ratio);
        }
    }
}

#[test]
fn qawo_and_qawf_match_compiled_gsl() {
    let mut seq: Option<QawoTable> = None;
    let (mut tbl_rows, mut mom, mut mom_exact) = (0usize, 0usize, 0usize);
    let (mut o, mut fq) = (Tally::default(), Tally::default());
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "TBL" => {
                let tag = f[1];
                let (omega, len): (f64, f64) = (f[2].parse().unwrap(), f[3].parse().unwrap());
                let sine = f[4] == "1";
                let n: usize = f[5].parse().unwrap();
                let want: Vec<f64> = f[6..].iter().map(|s| s.parse().unwrap()).collect();
                assert_eq!(want.len(), 25 * n, "{line}");
                let t = match tag {
                    "seq0" => {
                        seq = Some(QawoTable::new(3.0, 1.0, QawoKind::Cosine, 3).unwrap());
                        seq.as_ref().unwrap().clone()
                    }
                    "seq_same_len" => {
                        seq.as_mut().unwrap().set_length(1.0);
                        seq.as_ref().unwrap().clone()
                    }
                    "seq_len2.5" => {
                        seq.as_mut().unwrap().set_length(2.5);
                        seq.as_ref().unwrap().clone()
                    }
                    "seq_set" => {
                        seq.as_mut().unwrap().set(7.0, 0.5, QawoKind::Sine);
                        seq.as_ref().unwrap().clone()
                    }
                    _ => QawoTable::new(omega, len, kind(sine), n).unwrap(),
                };
                assert_eq!((t.omega(), t.length(), t.kind() == QawoKind::Sine), (omega, len, sine), "{tag}");
                let mut row_exact = 0;
                for level in 0..n {
                    let m = t.moments(level).unwrap();
                    for (i, g) in m.iter().enumerate() {
                        mom += 1;
                        if g.to_bits() == want[25 * level + i].to_bits() {
                            mom_exact += 1;
                            row_exact += 1;
                        }
                    }
                }
                println!("  table {tag}: moments {row_exact}/{}", 25 * n);
                tbl_rows += 1;
            }
            "QAWO" => {
                let name = f[1];
                let status: i32 = f[2].parse().unwrap();
                let want = (f[3].parse::<f64>().unwrap(), f[4].parse::<f64>().unwrap());
                let (size, ne): (usize, usize) = (f[5].parse().unwrap(), f[6].parse().unwrap());
                let (fname, a, omega, len, sine, levels, ea, er, limit) = qawo_case(name);
                let t = QawoTable::new(omega, len, kind(sine), levels).unwrap();
                let calls = Cell::new(0usize);
                let g = integrand(fname);
                let out = qawo_with_status(
                    |x| {
                        calls.set(calls.get() + 1);
                        g(x)
                    },
                    a,
                    ea,
                    er,
                    limit,
                    &t,
                );
                o.rows += 1;
                if out.gsl_status == status {
                    o.status += 1;
                } else {
                    println!("  {name}: status {} != GSL {status}", out.gsl_status);
                }
                if out.integral.subintervals == size {
                    o.size += 1;
                } else {
                    println!("  {name}: size {} != GSL {size}", out.integral.subintervals);
                }
                if calls.get() == ne {
                    o.neval += 1;
                } else {
                    println!("  {name}: neval {} != GSL {ne}", calls.get());
                }
                o.note(name, (out.integral.value, out.integral.abserr), want);
            }
            "QAWF" => {
                let name = f[1];
                let status: i32 = f[2].parse().unwrap();
                let want = (f[3].parse::<f64>().unwrap(), f[4].parse::<f64>().unwrap());
                let size: usize = f[5].parse().unwrap();
                let ne: usize = f[7].parse().unwrap();
                let (fname, a, omega, sine, ea, limit, climit) = qawf_case(name);
                let mut t = QawoTable::new(omega, 1.0, kind(sine), 25).unwrap();
                let calls = Cell::new(0usize);
                let g = integrand(fname);
                let out = qawf_with_status(
                    |x| {
                        calls.set(calls.get() + 1);
                        g(x)
                    },
                    a,
                    ea,
                    limit,
                    climit,
                    &mut t,
                );
                fq.rows += 1;
                if out.gsl_status == status {
                    fq.status += 1;
                } else {
                    println!("  {name}: status {} != GSL {status}", out.gsl_status);
                }
                if out.integral.subintervals == size {
                    fq.size += 1;
                } else {
                    println!("  {name}: size {} != GSL {size}", out.integral.subintervals);
                }
                if calls.get() == ne {
                    fq.neval += 1;
                } else {
                    println!("  {name}: neval {} != GSL {ne}", calls.get());
                }
                fq.note(name, (out.integral.value, out.integral.abserr), want);
            }
            other => panic!("unknown row {other}"),
        }
    }
    println!(
        "TABLES {tbl_rows}: moments {mom_exact}/{mom}\nQAWO {}: status {} size {} neval {} result {} abserr {} worst rel {:.2e} worst abserr ratio {:.4}\nQAWF {}: status {} size {} neval {} result {} abserr {} worst rel {:.2e} worst abserr ratio {:.4}",
        o.rows, o.status, o.size, o.neval, o.result_exact, o.abserr_exact, o.worst_result, o.worst_abserr_ratio,
        fq.rows, fq.status, fq.size, fq.neval, fq.result_exact, fq.abserr_exact, fq.worst_result, fq.worst_abserr_ratio
    );
    assert_eq!(tbl_rows, 12);
    assert_eq!((o.rows, fq.rows), (28, 18));
    // Pass criteria, fixed before the run (P11, P12).
    assert_eq!((o.status, o.size, o.neval), (o.rows, o.rows, o.rows), "QAWO status/size/neval");
    assert_eq!((fq.status, fq.size, fq.neval), (fq.rows, fq.rows, fq.rows), "QAWF status/size/neval");
    assert!(o.worst_result <= 1e-8 && fq.worst_result <= 1e-8);
}
