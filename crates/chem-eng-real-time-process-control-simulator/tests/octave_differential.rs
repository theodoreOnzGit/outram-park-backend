//! Differential comparison of `beta_testing::z_domain` against a run of the GNU
//! Octave control package (gh:#1433, epic #1336).
//!
//! Methodology, upstream version, numbers and defects:
//! `verification_and_validation/octave_differential/` (`README.md`, `PREDICTIONS.md`
//! committed before the first run, `RESULTS.md`). `octave_reference.csv` was produced
//! by `octave_driver.m` (Octave 8.4.0, control 4.0.1) with the inputs printed into the
//! same file; this test reads inputs and outputs from it and recomputes every case.
//!
//! Set `OCTAVE_DIFF_PRINT=1` with `--nocapture` to print the tables.
//! Research, education and V&V only.

use std::collections::BTreeMap;

use chem_eng_real_time_process_control_simulator::beta_testing::z_domain::{
    C2dMethod, ContinuousTransferFn, D2cMethod, DiscreteTransferFn,
};
use uom::si::angular_velocity::radian_per_second;
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::time::second;

const REFERENCE: &str =
    include_str!("../verification_and_validation/octave_differential/octave_reference.csv");

const REFERENCE_430: &str = include_str!(
    "../verification_and_validation/octave_differential/octave_reference_pkg430_matched.csv"
);

/// (tag, name, key) -> values
type Table = BTreeMap<(String, String, String), Vec<f64>>;

fn print_enabled() -> bool {
    std::env::var("OCTAVE_DIFF_PRINT").is_ok()
}

fn parse() -> (Table, Vec<String>) {
    parse_text(REFERENCE)
}

fn parse_text(text: &str) -> (Table, Vec<String>) {
    let mut t = Table::new();
    let mut errs = Vec::new();
    for line in text.lines() {
        let mut it = line.splitn(3, '|');
        let tag = it.next().unwrap_or("").to_string();
        let name = it.next().unwrap_or("").to_string();
        let rest = it.next().unwrap_or("");
        if tag.ends_with("_err") {
            errs.push(line.to_string());
            continue;
        }
        let (key, vals) = rest.rsplit_once('|').expect("key|values");
        let v: Vec<f64> = vals
            .split_whitespace()
            .map(|x| x.parse::<f64>().expect("f64"))
            .collect();
        t.insert((tag, name, key.to_string()), v);
    }
    (t, errs)
}

fn get<'a>(t: &'a Table, tag: &str, name: &str, key: &str) -> &'a Vec<f64> {
    t.get(&(tag.to_string(), name.to_string(), key.to_string()))
        .unwrap_or_else(|| panic!("missing {tag}|{name}|{key}"))
}

fn names(t: &Table, tag: &str) -> Vec<String> {
    let mut n: Vec<String> = t
        .keys()
        .filter(|k| k.0 == tag)
        .map(|k| k.1.clone())
        .collect();
    n.dedup();
    n
}

/// Comparison statistics between a port sequence and an Octave sequence.
#[derive(Debug, Clone, Copy, Default)]
struct Cmp {
    n: usize,
    bit_identical: usize,
    /// max |p - o| / max |o|
    normwise: f64,
    /// max over entries with o != 0 of |p - o| / |o|
    per_entry: f64,
}

fn compare(p: &[f64], o: &[f64]) -> Cmp {
    assert_eq!(p.len(), o.len(), "length mismatch");
    let scale = o.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let mut c = Cmp {
        n: p.len(),
        ..Cmp::default()
    };
    for (a, b) in p.iter().zip(o) {
        if a.to_bits() == b.to_bits() || (a == b) {
            c.bit_identical += 1;
            continue;
        }
        if scale > 0.0 {
            c.normwise = c.normwise.max((a - b).abs() / scale);
        }
        if *b != 0.0 {
            c.per_entry = c.per_entry.max(((a - b) / b).abs());
        } else {
            c.per_entry = f64::INFINITY;
        }
    }
    c
}

fn merge(a: Cmp, b: Cmp) -> Cmp {
    Cmp {
        n: a.n + b.n,
        bit_identical: a.bit_identical + b.bit_identical,
        normwise: a.normwise.max(b.normwise),
        per_entry: a.per_entry.max(b.per_entry),
    }
}

fn run_port(
    b: &[f64],
    a: &[f64],
    ts: f64,
    u: &[f64],
) -> Vec<f64> {
    let mut sys = DiscreteTransferFn::from_z_inverse_coefficients(
        b.to_vec(),
        a.to_vec(),
        Time::new::<second>(ts),
    )
    .expect("valid discrete tf");
    u.iter()
        .map(|&x| sys.advance_one_sample(Ratio::new::<ratio>(x)).get::<ratio>())
        .collect()
}

/// Case 1 results by comparison kind.
struct Case1 {
    vs_filter: Cmp,
    vs_portrep: Cmp,
    vs_octrep: Cmp,
    octrep_vs_filter: Cmp,
    vs_lsim: Cmp,
    vs_step_impulse: Cmp,
    per_system: Vec<(String, Cmp, Cmp)>,
}

fn case1(t: &Table) -> Case1 {
    let mut r = Case1 {
        vs_filter: Cmp::default(),
        vs_portrep: Cmp::default(),
        vs_octrep: Cmp::default(),
        octrep_vs_filter: Cmp::default(),
        vs_lsim: Cmp::default(),
        vs_step_impulse: Cmp::default(),
        per_system: Vec::new(),
    };
    for name in names(t, "sysd") {
        let b = get(t, "sysd", &name, "b_zinv");
        let a = get(t, "sysd", &name, "a_zinv");
        let ts = get(t, "sysd", &name, "T")[0];
        let mut sys_filter = Cmp::default();
        let mut sys_lsim = Cmp::default();
        for input in ["rand", "step", "impulse"] {
            let u = get(t, "u", input, "u");
            let y = run_port(b, a, ts, u);
            let f = compare(&y, get(t, "filter", &name, input));
            r.vs_filter = merge(r.vs_filter, f);
            sys_filter = merge(sys_filter, f);
            r.vs_portrep = merge(r.vs_portrep, compare(&y, get(t, "portrep", &name, input)));
            r.vs_octrep = merge(r.vs_octrep, compare(&y, get(t, "octrep", &name, input)));
            r.octrep_vs_filter = merge(
                r.octrep_vs_filter,
                compare(get(t, "octrep", &name, input), get(t, "filter", &name, input)),
            );
            let l = compare(&y, get(t, "lsim", &name, input));
            r.vs_lsim = merge(r.vs_lsim, l);
            sys_lsim = merge(sys_lsim, l);
            if input != "rand" {
                let tag = if input == "step" { "step_fn" } else { "impulse_fn" };
                r.vs_step_impulse =
                    merge(r.vs_step_impulse, compare(&y, get(t, tag, &name, input)));
            }
        }
        r.per_system.push((name, sys_filter, sys_lsim));
    }
    r
}

fn print_cmp(label: &str, c: Cmp) {
    println!(
        "{label:34} n={:5} bit-identical={:5} normwise={:.3e} per-entry={:.3e}",
        c.n, c.bit_identical, c.normwise, c.per_entry
    );
}

#[test]
fn case1_discrete_simulation_vs_octave() {
    let (t, _) = parse();
    let r = case1(&t);
    if print_enabled() {
        print_cmp("port vs filter()", r.vs_filter);
        print_cmp("port vs Octave-order replica", r.vs_octrep);
        print_cmp("port vs port-order replica", r.vs_portrep);
        print_cmp("octave replica vs filter()", r.octrep_vs_filter);
        print_cmp("port vs lsim (ss route)", r.vs_lsim);
        print_cmp("port vs step()/impulse()", r.vs_step_impulse);
        for (n, f, l) in &r.per_system {
            print_cmp(&format!("  {n} filter"), *f);
            print_cmp(&format!("  {n} lsim"), *l);
        }
    }
    // Operation-order proof: the replica of the port's order is the port.
    assert_eq!(r.vs_portrep.bit_identical, r.vs_portrep.n);
    // Operation-order proof: the replica of liboctave's order is the builtin.
    assert_eq!(r.octrep_vs_filter.bit_identical, r.octrep_vs_filter.n);
    assert_eq!(r.vs_filter.n, 13 * 3 * 40);
    // Pinned observations (2026-11-13, Octave 8.4.0 / control 4.0.1). Counts are exact;
    // the bounds are the PREDICTIONS.md ones (P1 normwise 1e-14, P4 normwise 1e-12).
    assert_eq!(r.vs_filter.bit_identical, 1019);
    assert!(r.vs_filter.normwise <= 1e-14, "{}", r.vs_filter.normwise);
    assert_eq!(r.vs_lsim.bit_identical, 449);
    assert!(r.vs_lsim.normwise <= 1e-12, "{}", r.vs_lsim.normwise);
    assert_eq!(r.vs_step_impulse.bit_identical, 312);
    assert!(r.vs_step_impulse.normwise <= 1e-12);
}

// ------------------------------------------------------------------ c2d / d2c

fn method_of(m: &str) -> C2dMethod {
    match m {
        "zoh" => C2dMethod::Zoh,
        "tustin" => C2dMethod::Tustin,
        "prewarp" => C2dMethod::TustinPrewarp {
            prewarp_frequency: AngularVelocity::new::<radian_per_second>(1.0),
        },
        _ => C2dMethod::MatchedPoleZero,
    }
}

fn pad_end(v: &[f64], n: usize) -> Vec<f64> {
    let mut o = v.to_vec();
    o.resize(n, 0.0);
    o
}

/// Octave descending-z polynomial -> z^-1-ascending of length `n` (pad at the front).
fn octave_desc_to_zinv(v: &[f64], n: usize) -> Vec<f64> {
    let mut o = vec![0.0; n - v.len()];
    o.extend_from_slice(v);
    o
}

/// One c2d row: (name, key, comparison of num+den, port result is an error).
struct C2dRow {
    name: String,
    key: String,
    cmp: Option<Cmp>,
    port_error: Option<String>,
}

fn c2d_rows(t: &Table) -> Vec<C2dRow> {
    let mut rows = Vec::new();
    for ((tag, name, key), onum) in t.iter() {
        if tag != "c2d_num" {
            continue;
        }
        let oden = get(t, "c2d_den", name, key);
        let (m, tpart) = key.split_once('|').expect("method|T=");
        let ts: f64 = tpart.trim_start_matches("T=").parse().expect("T");
        let num = get(t, "sysc", name, "num_asc").clone();
        let den = get(t, "sysc", name, "den_asc").clone();
        let sys = ContinuousTransferFn::new(num, den).expect("continuous tf");
        match sys.to_discrete(Time::new::<second>(ts), method_of(m)) {
            Ok(d) => {
                let n = oden.len().max(d.denominator_z_inverse().len());
                let on = octave_desc_to_zinv(onum, n);
                let od = octave_desc_to_zinv(oden, n);
                let pn = pad_end(d.numerator_z_inverse(), n);
                let pd = pad_end(d.denominator_z_inverse(), n);
                rows.push(C2dRow {
                    name: name.clone(),
                    key: key.clone(),
                    cmp: Some(merge(compare(&pn, &on), compare(&pd, &od))),
                    port_error: None,
                });
            }
            Err(e) => rows.push(C2dRow {
                name: name.clone(),
                key: key.clone(),
                cmp: None,
                port_error: Some(format!("{e}")),
            }),
        }
    }
    rows
}

#[test]
fn case2_c2d_vs_octave() {
    let (t, _) = parse();
    let rows = c2d_rows(&t);
    let mut by_method: BTreeMap<String, Cmp> = BTreeMap::new();
    let mut gaps = Vec::new();
    for r in &rows {
        if print_enabled() {
            match (&r.cmp, &r.port_error) {
                (Some(c), _) => print_cmp(&format!("{} {}", r.name, r.key), *c),
                (_, Some(e)) => println!("{} {} PORT ERROR: {e}", r.name, r.key),
                _ => {}
            }
        }
        match (&r.cmp, &r.port_error) {
            (Some(c), _) => {
                let m = r.key.split('|').next().unwrap_or("").to_string();
                let e = by_method.entry(m).or_default();
                *e = merge(*e, *c);
            }
            (_, Some(_)) => gaps.push(format!("{} {}", r.name, r.key)),
            _ => {}
        }
    }
    if print_enabled() {
        for (m, c) in &by_method {
            print_cmp(&format!("c2d {m}"), *c);
        }
        println!("port refused (Octave succeeds): {gaps:?}");
    }
    assert!(!rows.is_empty());
    let get_m = |m: &str| by_method.get(m).copied().unwrap();
    // exact bit-identical counts (n is coefficients compared)
    assert_eq!((get_m("tustin").bit_identical, get_m("tustin").n), (46, 100));
    assert_eq!((get_m("zoh").bit_identical, get_m("zoh").n), (45, 84));
    assert_eq!((get_m("prewarp").bit_identical, get_m("prewarp").n), (31, 100));
    assert_eq!((get_m("matched").bit_identical, get_m("matched").n), (69, 84));
    // PREDICTIONS.md bounds that held: P5 Tustin 1e-14, P6 ZOH 1e-13 (per entry).
    assert!(get_m("tustin").per_entry <= 1e-14);
    assert!(get_m("zoh").per_entry <= 1e-13);
    // P7 (prewarp 1e-14) MISSED: worst 1.5e-13 per entry on the ill-conditioned c08
    // third-order case at T = 0.1. Pinned at the observed value, not a relaxed gate.
    assert!(get_m("prewarp").per_entry <= 2e-13);
    // P8 (matched 1e-14) MISSED against control 4.0.1 on c09 only (pole at s = 0): the
    // reference differs by an upstream change, see case2_c2d_vs_pkg_control_430_matched.
    assert!(get_m("matched").per_entry > 1e-2);
    assert_eq!(gaps.len(), 4, "port refuses order 3 ZOH/matched; Octave succeeds");
}

fn d2c_rows(t: &Table, errs: &[String]) -> (BTreeMap<String, Cmp>, Vec<String>, Vec<String>) {
    let mut by_method: BTreeMap<String, Cmp> = BTreeMap::new();
    let mut port_errors = Vec::new();
    let mut oct_errors = Vec::new();
    for e in errs {
        if e.starts_with("d2c_err") {
            oct_errors.push(e.split('|').take(3).collect::<Vec<_>>().join("|"));
        }
    }
    for ((tag, name, key), onum) in t.iter() {
        if tag != "d2c_num" {
            continue;
        }
        let (m, tpart) = key.split_once('|').expect("method|T=");
        if m == "zoh" {
            continue; // port deliberately omits d2c ZOH
        }
        let oden = get(t, "d2c_den", name, key);
        let ts: f64 = tpart.trim_start_matches("T=").parse().expect("T");
        let b = get(t, "sysd_d2c", name, "b_zinv").clone();
        let a = get(t, "sysd_d2c", name, "a_zinv").clone();
        let sys = DiscreteTransferFn::from_z_inverse_coefficients(b, a, Time::new::<second>(ts))
            .expect("discrete tf");
        let method = match m {
            "tustin" => D2cMethod::Tustin,
            "prewarp" => D2cMethod::TustinPrewarp {
                prewarp_frequency: AngularVelocity::new::<radian_per_second>(1.0),
            },
            _ => D2cMethod::MatchedPoleZero,
        };
        match sys.to_continuous(method) {
            Ok(c) => {
                // port: ascending s; Octave: descending s. Normalise by the leading
                // denominator coefficient on both sides.
                let n = oden.len().max(c.denominator_ascending_s().len());
                let mut od: Vec<f64> = oden.clone();
                let mut on: Vec<f64> = onum.clone();
                let lead_o = od[0];
                od.iter_mut().for_each(|x| *x /= lead_o);
                on.iter_mut().for_each(|x| *x /= lead_o);
                let on = octave_desc_to_zinv(&on, n); // front-pad == descending s
                let od = octave_desc_to_zinv(&od, n);
                let mut pn: Vec<f64> = pad_end(c.numerator_ascending_s(), n);
                let mut pd: Vec<f64> = pad_end(c.denominator_ascending_s(), n);
                pn.reverse();
                pd.reverse();
                let lead_p = pd[0];
                pn.iter_mut().for_each(|x| *x /= lead_p);
                pd.iter_mut().for_each(|x| *x /= lead_p);
                let cmp = merge(compare(&pn, &on), compare(&pd, &od));
                if print_enabled() {
                    print_cmp(&format!("d2c {name} {key}"), cmp);
                }
                let e = by_method.entry(m.to_string()).or_default();
                *e = merge(*e, cmp);
            }
            Err(e) => port_errors.push(format!("{name} {key}: {e}")),
        }
    }
    (by_method, port_errors, oct_errors)
}

#[test]
fn case2_d2c_vs_octave() {
    let (t, errs) = parse();
    let (by_method, port_errors, oct_errors) = d2c_rows(&t, &errs);
    if print_enabled() {
        for (m, c) in &by_method {
            print_cmp(&format!("d2c {m}"), *c);
        }
        println!("port errors: {port_errors:?}\nOctave errors: {oct_errors:?}");
    }
    assert!(!by_method.is_empty());
    for (m, bits, n) in [("tustin", 8, 24), ("prewarp", 8, 24), ("matched", 5, 6)] {
        let c = by_method[m];
        assert_eq!((c.bit_identical, c.n), (bits, n), "{m}");
        // P12: 1e-13 after normalising by the leading denominator coefficient
        assert!(c.normwise <= 1e-13, "{m} {}", c.normwise);
    }
    assert!(port_errors.is_empty());
}

#[test]
fn case3_dcgain_vs_octave() {
    let (t, _) = parse();
    let mut identical = 0;
    let mut total = 0;
    for name in names(&t, "sysc") {
        let num = get(&t, "sysc", &name, "num_asc").clone();
        let den = get(&t, "sysc", &name, "den_asc").clone();
        let sys = ContinuousTransferFn::new(num, den).expect("tf");
        let o = get(&t, "dcgain", &name, "dcgain")[0];
        let p = sys.steady_state_gain();
        total += 1;
        if p == o || p.to_bits() == o.to_bits() {
            identical += 1;
        }
        if print_enabled() {
            println!("dcgain {name}: port {p:.17e} octave {o:.17e}");
        }
    }
    assert_eq!((identical, total), (9, 9)); // P11, including Inf for the integrator
}

/// The same c2d comparison against the pkg-control 4.3.0 `@tf/__c2d__.m` (matched
/// branch uses `s = j*w_c`, as the port does), shadowing the installed 4.0.1.
#[test]
fn case2_c2d_vs_pkg_control_430_matched() {
    let (t, _) = parse_text(REFERENCE_430);
    let rows = c2d_rows(&t);
    let mut matched = Cmp::default();
    for r in &rows {
        if r.key.starts_with("matched") {
            if let Some(c) = r.cmp {
                if print_enabled() {
                    print_cmp(&format!("430 {} {}", r.name, r.key), c);
                }
                matched = merge(matched, c);
            }
        }
    }
    if print_enabled() {
        print_cmp("430 c2d matched (all)", matched);
    }
    assert_eq!((matched.bit_identical, matched.n), (73, 84));
    assert!(matched.per_entry <= 1e-14, "{}", matched.per_entry);
}

/// Where Octave refuses a matched `d2c` (zero or pole at z = 0), the port must refuse too.
#[test]
fn case2_d2c_error_parity() {
    let (t, errs) = parse();
    let mut n = 0;
    for e in errs.iter().filter(|e| e.starts_with("d2c_err|") && e.contains("|matched|")) {
        let name = e.split('|').nth(1).unwrap().to_string();
        let b = get(&t, "sysd_d2c", &name, "b_zinv").clone();
        let a = get(&t, "sysd_d2c", &name, "a_zinv").clone();
        let sys = DiscreteTransferFn::from_z_inverse_coefficients(b, a, Time::new::<second>(0.5))
            .unwrap();
        let r = sys.to_continuous(D2cMethod::MatchedPoleZero);
        if print_enabled() {
            println!("d2c matched {name}: port {:?}", r.as_ref().err());
        }
        assert!(r.is_err(), "{name}: Octave refuses, port must too");
        n += 1;
    }
    assert_eq!(n, 3);
}
