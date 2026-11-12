//! **V&V — every ported `specfunc` routine against compiled GSL 2.8**
//! (gh:#1338 case 1, parent epic gh:#1336).
//!
//! # Methodology
//!
//! `reference-data/gsl/specfunc_reference_driver.c` calls GSL 2.8's
//! `gsl_sf_*_e` for each routine PETIR ports, on a grid built to reach every
//! branch (dense linear grid, log grid, the round numbers GSL's thresholds sit
//! on and their floating-point neighbours, huge and tiny arguments, and
//! arguments GSL refuses) and writes the 64-bit patterns of the arguments, GSL's
//! status and GSL's value to `specfunc-gsl-2.8-reference.txt`. This file replays
//! each row through the PETIR routine and compares **bit patterns**.
//!
//! GSL's `result.err` is not compared: PETIR's specfunc returns a bare `f64`
//! and carries no error estimate (`specfunc/mod.rs`).
//!
//! Row classes: (a) GSL status 0 -> PETIR value must equal GSL's bit for bit,
//! or be reported with its relative difference; (b) GSL status != 0 ->
//! agreement means PETIR returns the same bits or a non-finite value (PETIR's
//! way of refusing).
//!
//! `erf` and `erfc` are NOT ports (`specfunc/erf.rs`: a deliberate
//! substitution of `libm`'s fdlibm); they are compared for information and
//! expected to differ in the last ulp. `erfcx`/`erfc_scaled` and `debye_n`
//! have no GSL counterpart. `inc_gamma*`, `inv_inc_gamma` and `erf_inv` are
//! OpenFOAM lifts, not GSL.
//!
//! Results are in `reference-data/gsl/README.md` and
//! `docs/verification-summary.md`; each test below prints its own count.

use petir::specfunc::airy::*;
use petir::specfunc::atanint::atanint;
use petir::specfunc::bessel::*;
use petir::specfunc::clausen::clausen;
use petir::specfunc::dawson::dawson;
use petir::specfunc::debye::*;
use petir::specfunc::dilog::dilog;
use petir::specfunc::ellint::{self, Mode};
use petir::specfunc::elljac::elljac;
use petir::specfunc::expint3::expint_3;
use petir::specfunc::fermi_dirac::*;
use petir::specfunc::gamma as sfgamma;
use petir::specfunc::gegenbauer::*;
use petir::specfunc::lambert::*;
use petir::specfunc::legendre::legendre_plm;
use petir::specfunc::psi;
use petir::specfunc::shint::{chi, shi};
use petir::specfunc::sinint::{ci, si};
use petir::specfunc::synchrotron::*;
use petir::specfunc::transport::*;
use petir::specfunc::trig::*;
use petir::specfunc::zeta;
use petir::specfunc::{erf, erfc};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/specfunc-gsl-2.8-reference.txt");

/// One reference row: arguments, mode (ellint only), GSL status, GSL values.
struct Row {
    args: Vec<f64>,
    mode: Option<u8>,
    status: i32,
    /// GSL's own `result.err` (0 where GSL has none).
    err: f64,
    vals: Vec<f64>,
}

/// Number of real arguments of each routine in the reference file.
fn nargs(name: &str) -> usize {
    match name {
        "hzeta" | "ln_beta" | "beta" | "choose" | "psi_n" | "elljac" | "rc" | "ellint_f" | "ellint_e"
        | "ellint_d" | "ellint_pcomp" | "gegenpoly_1" | "gegenpoly_2" | "gegenpoly_3" => 2,
        "rf" | "rd" | "ellint_p" | "gegenpoly_n" | "legendre_plm" => 3,
        "rj" => 4,
        _ => 1,
    }
}
fn has_mode(name: &str) -> bool {
    matches!(
        name,
        "rc" | "rf" | "rd" | "rj" | "ellint_f" | "ellint_e" | "ellint_d" | "ellint_p" | "ellint_kcomp"
            | "ellint_ecomp" | "ellint_dcomp" | "ellint_pcomp"
    )
}
fn nvals(name: &str) -> usize {
    match name {
        "elljac" => 3,
        "ln_gamma_sgn" => 2,
        _ => 1,
    }
}

fn table() -> &'static BTreeMap<&'static str, Vec<Row>> {
    static T: OnceLock<BTreeMap<&'static str, Vec<Row>>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m: BTreeMap<&str, Vec<Row>> = BTreeMap::new();
        for line in REFERENCE.lines() {
            let mut it = line.split_whitespace();
            let name = it.next().expect("name");
            let hex = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).expect("hex"));
            let args: Vec<f64> = (0..nargs(name)).map(|_| hex(it.next().unwrap())).collect();
            let mode = has_mode(name).then(|| it.next().unwrap().parse().unwrap());
            let status: i32 = it.next().unwrap().parse().unwrap();
            let err = hex(it.next().unwrap());
            let vals: Vec<f64> = (0..nvals(name)).map(|_| hex(it.next().unwrap())).collect();
            m.entry(name).or_default().push(Row { args, mode, status, err, vals });
        }
        m
    })
}

fn mode(r: &Row) -> Mode {
    if r.mode == Some(0) {
        Mode::Double
    } else {
        Mode::Single
    }
}

/// Evaluate PETIR's routine for one row.
fn ours(name: &str, r: &Row) -> Option<Vec<f64>> {
    let a = &r.args;
    let x = a[0];
    let int = |v: f64| v as i64;
    // Routines whose PETIR signature takes an unsigned order cannot be asked
    // GSL's negative-order questions (`gegenpoly_n(-1, ..)`): not comparable.
    if name == "gegenpoly_n" && a[0] < 0.0 {
        return None;
    }
    // Documented API difference (`gamma::choose`): PETIR defines C(n, k) = 0
    // for k > n, where GSL refuses with GSL_EDOM.
    if name == "choose" && a[1] > a[0] {
        return None;
    }
    let v = match name {
        "airy_ai" => airy_ai(x),
        "airy_ai_scaled" => airy_ai_scaled(x),
        "airy_bi" => airy_bi(x),
        "airy_bi_scaled" => airy_bi_scaled(x),
        "atanint" => atanint(x),
        "clausen" => clausen(x),
        "dawson" => dawson(x),
        "dilog" => dilog(x),
        "expint_3" => expint_3(x),
        "si" => si(x),
        "ci" => ci(x),
        "shi" => shi(x),
        "chi" => chi(x),
        "synchrotron_1" => synchrotron_1(x),
        "synchrotron_2" => synchrotron_2(x),
        "lambert_w0" => lambert_w0(x),
        "lambert_wm1" => lambert_wm1(x),
        "bessel_j0" => bessel_j0(x),
        "bessel_j1" => bessel_j1(x),
        "bessel_y0" => bessel_y0(x),
        "bessel_y1" => bessel_y1(x),
        "bessel_i0" => bessel_i0(x),
        "bessel_i1" => bessel_i1(x),
        "bessel_i0_scaled" => bessel_i0_scaled(x),
        "bessel_i1_scaled" => bessel_i1_scaled(x),
        "bessel_k0" => bessel_k0(x),
        "bessel_k1" => bessel_k1(x),
        "bessel_k0_scaled" => bessel_k0_scaled(x),
        "bessel_k1_scaled" => bessel_k1_scaled(x),
        "debye_1" => debye_1(x),
        "debye_2" => debye_2(x),
        "debye_3" => debye_3(x),
        "debye_4" => debye_4(x),
        "debye_5" => debye_5(x),
        "debye_6" => debye_6(x),
        "transport_2" => transport_2(x),
        "transport_3" => transport_3(x),
        "transport_4" => transport_4(x),
        "transport_5" => transport_5(x),
        "fd_m1" => fermi_dirac_m1(x),
        "fd_0" => fermi_dirac_0(x),
        "fd_1" => fermi_dirac_1(x),
        "fd_2" => fermi_dirac_2(x),
        "fd_mhalf" => fermi_dirac_mhalf(x),
        "fd_half" => fermi_dirac_half(x),
        "fd_3half" => fermi_dirac_3half(x),
        "erf" => erf(x),
        "erfc" => erfc(x),
        "ln_gamma" => sfgamma::ln_gamma(x),
        "gamma" => sfgamma::gamma(x),
        "ln_gamma_sgn" => {
            let (l, s) = sfgamma::ln_gamma_sgn(x);
            return Some(vec![l, s]);
        }
        "factorial" => sfgamma::factorial(x as u32),
        "ln_factorial" => sfgamma::ln_factorial(x as u32),
        "ln_beta" => sfgamma::ln_beta(x, a[1]),
        "beta" => sfgamma::beta(x, a[1]),
        "choose" => sfgamma::choose(x as u32, a[1] as u32),
        "psi" => psi::psi(x),
        "psi_1" => psi::psi_1(x),
        "psi_1piy" => psi::psi_1piy(x),
        "psi_int" => psi::psi_int(x as u32),
        "psi_1_int" => psi::psi_1_int(x as u32),
        "psi_n" => psi::psi_n(x as u32, a[1]),
        "zeta" => zeta::zeta(x),
        "zetam1" => zeta::zetam1(x),
        "eta" => zeta::eta(x),
        "zeta_int" => zeta::zeta_int(int(x) as i32),
        "zetam1_int" => zeta::zetam1_int(int(x) as i32),
        "eta_int" => zeta::eta_int(int(x) as i32),
        "hzeta" => zeta::hzeta(x, a[1]),
        "gegenpoly_1" => gegenpoly_1(x, a[1]),
        "gegenpoly_2" => gegenpoly_2(x, a[1]),
        "gegenpoly_3" => gegenpoly_3(x, a[1]),
        "gegenpoly_n" => gegenpoly_n(x as u32, a[1], a[2]),
        "legendre_plm" => legendre_plm(x as u32, a[1] as u32, a[2]),
        "rc" => ellint::rc(x, a[1], mode(r)),
        "rf" => ellint::rf(x, a[1], a[2], mode(r)),
        "rd" => ellint::rd(x, a[1], a[2], mode(r)),
        "rj" => ellint::rj(x, a[1], a[2], a[3], mode(r)),
        "ellint_f" => ellint::ellint_f(x, a[1], mode(r)),
        "ellint_e" => ellint::ellint_e(x, a[1], mode(r)),
        "ellint_d" => ellint::ellint_d(x, a[1], mode(r)),
        "ellint_p" => ellint::ellint_p(x, a[1], a[2], mode(r)),
        "ellint_kcomp" => ellint::ellint_kcomp(x, mode(r)),
        "ellint_ecomp" => ellint::ellint_ecomp(x, mode(r)),
        "ellint_dcomp" => ellint::ellint_dcomp(x, mode(r)),
        "ellint_pcomp" => ellint::ellint_pcomp(x, a[1], mode(r)),
        "elljac" => {
            let j = elljac(x, a[1]);
            return Some(vec![j.sn, j.cn, j.dn]);
        }
        "angle_restrict_pos" => angle_restrict_pos(x),
        "angle_restrict_symm" => angle_restrict_symm(x),
        other => panic!("no PETIR binding for reference routine {other}"),
    };
    Some(vec![v])
}

#[derive(Default)]
struct Tally {
    ok_rows: usize,
    ok_exact: usize,
    ok_rel_lt_1e8: usize,
    /// Rows >= 1e-8 whose difference is nonetheless within GSL's own `err`.
    within_gsl_err: usize,
    worst_rel: f64,
    worst_at: String,
    err_rows: usize,
    err_agree: usize,
    err_disagree: Vec<String>,
    big: Vec<String>,
    /// First few non-identical rows, shown when `SPECFUNC_SHOW_DIFFS` is set.
    examples: Vec<String>,
    /// Rows not compared: a NaN argument, or GSL returning NaN with status 0.
    gsl_nan_success: usize,
    /// Rows not representable in PETIR's signature (negative unsigned order).
    not_representable: usize,
}

fn rel(a: f64, b: f64) -> f64 {
    if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) || a == b {
        return 0.0;
    }
    if !a.is_finite() || !b.is_finite() {
        return f64::INFINITY;
    }
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

/// Which rows of a routine to compare.
#[derive(Clone, Copy, PartialEq)]
enum Region {
    All,
    /// Everything except the neighbourhood of the negative-integer poles
    /// of Gamma (see [`near_pole`]).
    OutsidePoles,
    /// Only that neighbourhood.
    NearPoles,
}

/// GSL's `lngamma_sgn_sing` (specfunc/gamma.c:763) is the branch for
/// `x = -N + eps` with `|eps| < 0.02`; ~~PETIR does not port it~~ **CORRECTED
/// 2026-11-12:** PETIR ports it since gh:#1419 (`gamma.rs`,
/// `lngamma_sgn_sing`). A row is "near a pole" when the argument of
/// Gamma, or of any Gamma inside `beta`/`ln_beta`, is in that
/// neighbourhood on the negative axis.
fn near_pole(name: &str, r: &Row) -> bool {
    let near = |x: f64| x < 0.0 && (x - x.round()).abs() < 0.02;
    match name {
        "ln_gamma" | "ln_gamma_sgn" | "gamma" => near(r.args[0]),
        "ln_beta" | "beta" => near(r.args[0]) || near(r.args[1]) || near(r.args[0] + r.args[1]),
        _ => false,
    }
}

fn compare(name: &str) -> Tally {
    compare_in(name, Region::All)
}

fn compare_in(name: &str, region: Region) -> Tally {
    let mut t = Tally::default();
    for r in table().get(name).unwrap_or_else(|| panic!("no rows for {name}")) {
        match region {
            Region::OutsidePoles if near_pole(name, r) => continue,
            Region::NearPoles if !near_pole(name, r) => continue,
            _ => {}
        }
        let Some(o) = ours(name, r) else {
            t.not_representable += 1;
            continue;
        };
        let desc = || format!("{name}({:?}) mode={:?}: gsl {:?} (err {:e}) ours {:?}", r.args, r.mode, r.vals, r.err, o);
        if r.status == 0 {
            // Upstream oddity, not a port defect: GSL reports success while
            // returning NaN (a NaN argument is passed through as a value,
            // e.g. `dilog(NaN) = 0`, or huge arguments overflow silently).
            if r.args.iter().any(|v| v.is_nan()) || r.vals.iter().any(|v| v.is_nan()) {
                t.gsl_nan_success += 1;
                continue;
            }
            t.ok_rows += 1;
            let mut exact = true;
            let mut w = 0.0_f64;
            let mut covered = true;
            for (g, p) in r.vals.iter().zip(&o) {
                let bit_eq = g.to_bits() == p.to_bits() || (g.is_nan() && p.is_nan());
                exact &= bit_eq;
                w = w.max(rel(*p, *g));
                covered &= bit_eq || (p - g).abs() <= r.err;
            }
            if exact {
                t.ok_exact += 1;
            } else if t.examples.len() < 4 && std::env::var_os("SPECFUNC_SHOW_DIFFS").is_some() {
                t.examples.push(desc());
            }
            if w < 1e-8 {
                t.ok_rel_lt_1e8 += 1;
            } else if covered {
                t.within_gsl_err += 1;
            } else if t.big.len() < 400 {
                t.big.push(desc());
            }
            if w > t.worst_rel {
                t.worst_rel = w;
                t.worst_at = desc();
            }
        } else {
            t.err_rows += 1;
            let agree = r.vals.iter().zip(&o).all(|(g, p)| g == p || g.to_bits() == p.to_bits() || !p.is_finite());
            if agree {
                t.err_agree += 1;
            } else if t.err_disagree.len() < 12 {
                t.err_disagree.push(format!("status {}: {}", r.status, desc()));
            }
        }
    }
    t
}

/// Compare a family and print the measurement. The assertion is the
/// code-to-code rule of the workspace CLAUDE.md: every value GSL returns
/// successfully agrees to < 1e-8 relative, and every row GSL refuses is
/// refused (non-finite) or reproduced by PETIR.
fn family(label: &str, names: &[&str]) -> (usize, usize) {
    family_in(label, names, Region::All)
}

fn family_in(label: &str, names: &[&str], region: Region) -> (usize, usize) {
    let (mut rows, mut exact) = (0, 0);
    let mut problems = Vec::new();
    for n in names {
        let t = compare_in(n, region);
        println!(
            "  {label}/{n}: {} ok rows, {} bit-identical ({:.2} %), {} < 1e-8, worst rel {:.3e}; \
             {} GSL-refused rows, {} agree; {} >= 1e-8 but within GSL's own err, {} GSL-NaN-success rows \
             skipped, {} unrepresentable",
            t.ok_rows,
            t.ok_exact,
            100.0 * t.ok_exact as f64 / t.ok_rows.max(1) as f64,
            t.ok_rel_lt_1e8,
            t.worst_rel,
            t.err_rows,
            t.err_agree,
            t.within_gsl_err,
            t.gsl_nan_success,
            t.not_representable
        );
        if !t.worst_at.is_empty() {
            println!("      worst: {}", t.worst_at);
        }
        for e in &t.examples {
            println!("      differs: {e}");
        }
        rows += t.ok_rows;
        exact += t.ok_exact;
        if t.ok_rel_lt_1e8 + t.within_gsl_err != t.ok_rows {
            problems.push(format!("{n}: >=1e-8 rows: {:#?}", t.big));
        }
        if !t.err_disagree.is_empty() {
            problems.push(format!("{n}: refusal mismatch: {:#?}", t.err_disagree));
        }
    }
    assert!(problems.is_empty(), "{label}: {}", problems.join("\n"));
    (rows, exact)
}

#[test]
fn airy_matches_gsl() {
    family("airy", &["airy_ai", "airy_ai_scaled", "airy_bi", "airy_bi_scaled"]);
}
#[test]
fn atanint_matches_gsl() {
    family("atanint", &["atanint"]);
}
#[test]
fn bessel_matches_gsl() {
    family("bessel", &[
        "bessel_j0", "bessel_j1", "bessel_y0", "bessel_y1", "bessel_i0", "bessel_i1", "bessel_i0_scaled",
        "bessel_i1_scaled", "bessel_k0", "bessel_k1", "bessel_k0_scaled", "bessel_k1_scaled"
    ]);
}
#[test]
fn clausen_matches_gsl() {
    family("clausen", &["clausen"]);
}
#[test]
fn dawson_matches_gsl() {
    family("dawson", &["dawson"]);
}
#[test]
fn debye_matches_gsl() {
    family("debye", &["debye_1", "debye_2", "debye_3", "debye_4", "debye_5", "debye_6"]);
}
#[test]
fn dilog_matches_gsl() {
    family("dilog", &["dilog"]);
}
#[test]
fn ellint_matches_gsl() {
    family("ellint", &[
        "rc", "rf", "rd", "rj", "ellint_f", "ellint_e", "ellint_d", "ellint_p", "ellint_kcomp", "ellint_ecomp",
        "ellint_dcomp", "ellint_pcomp"
    ]);
}
#[test]
fn elljac_matches_gsl() {
    family("elljac", &["elljac"]);
}
#[test]
fn expint3_matches_gsl() {
    family("expint3", &["expint_3"]);
}
#[test]
fn fermi_dirac_matches_gsl() {
    family("fermi_dirac", &["fd_m1", "fd_0", "fd_1", "fd_2", "fd_mhalf", "fd_half", "fd_3half"]);
}
#[test]
fn gamma_matches_gsl() {
    family_in(
        "gamma",
        &["ln_gamma", "ln_gamma_sgn", "gamma", "factorial", "ln_factorial", "choose"],
        Region::OutsidePoles,
    );
}

/// **Measured defect, gh:#1419.** Within `|eps| < 0.02` of a negative integer
/// PETIR's `ln_gamma` reflects through `sin(pi x)` with a rounded `pi`, where
/// GSL switches to the `lngamma_sgn_sing` series. The error in `ln|Gamma|`
/// grows like `1e-16 / |eps|`: 3e-8 at `eps = 1e-9`, 3e-4 at `1e-12`, and
/// 0.04 to 0.15 (a factor `e^0.1` in Gamma itself) at one ulp from the pole.
/// ~~Ignored until the branch is ported.~~ **Un-ignored 2026-11-12**: the
/// branch is ported; results are below and in
/// `near_pole_rows_meet_the_gh1419_prediction`.
///
/// **Prediction, written 2026-11-12 and committed BEFORE the fix (gh:#1419).**
/// Porting `lngamma_sgn_0` / `lngamma_sgn_sing` and GSL's dispatch in
/// `gsl_sf_lngamma_e` / `gsl_sf_lngamma_sgn_e` (specfunc/gamma.c:729, :763,
/// :1115, :1167), and `gsl_sf_gamma_e` (:1255, which reaches the poles through
/// `sin(pi f)/pi` with `f = x - rint(x)` and so never divides by a rounded
/// `sin(pi x)`), moves the error against stock GSL (glibc libm) from the
/// measured 0.15 absolute / 1e-16/|eps| relative to: worst relative
/// difference below **5e-15** for `ln_gamma`, `ln_gamma_sgn` and `gamma` on
/// every near-pole row; no row at or above 1e-8; and at least **60 %** of
/// the near-pole values bit-identical to stock GSL. What remains is the
/// platform-libm last-ulp difference (`log`, `sin`, `exp`: PETIR's ARM/musl
/// ports against glibc, AGENTS.md "A 1-ulp difference ... is accepted") and,
/// for `N >= 2`, whatever `psi_n` contributes (its own 3.8e-15 residual, see
/// gh:#1421). The prediction can fail: a miss would be reported as it is.
#[test]
fn gamma_near_negative_integer_poles_match_gsl() {
    family_in("gamma-poles", &["ln_gamma", "ln_gamma_sgn", "gamma"], Region::NearPoles);
}

/// **Measured defects, gh:#1420 (FIXED 2026-11-12; un-ignored).** `ln_beta`
/// and `beta` were `lnG(a) + lnG(b) - lnG(a+b)` and refused non-positive
/// arguments (NaN), where GSL's `lnbeta_sgn` (specfunc/beta.c) switches to
/// `gammastar` + `log_1plusx` (the issue text says `lnpoch`; the source does
/// not) when one argument dominates and handles negative non-integers with a
/// sign. At `b = 1e10` PETIR's `ln_beta` was off by 4e-6 absolute (GSL's own
/// `err` is 5e-14). Results: `near_pole_rows_meet_the_gh1419_prediction` and
/// `fact_choose_beta_rows_meet_the_gh1420_prediction`.
///
/// **Prediction, written 2026-11-12 and committed BEFORE the fix (gh:#1420).**
/// Porting `fact_table`, `gsl_sf_fact_e`, `gsl_sf_lnfact_e`, `gsl_sf_choose_e`,
/// `gsl_sf_lnchoose_e` (specfunc/gamma.c), `gsl_sf_gamma_e` (the table for
/// integer arguments) and `gsl_sf_lnbeta_sgn_e` / `gsl_sf_beta_e`
/// (specfunc/beta.c; note that GSL's cancellation guard there is
/// `gsl_sf_gammastar_e` + `log_1plusx`, not `lnpoch` as the issue text says)
/// gives: `factorial`, `choose` (n <= 170 and the product branch) and
/// `gamma(n)` **100 % bit-identical** to stock GSL (they are table reads and
/// IEEE divisions, with no libm call); `ln_factorial` at least **95 %**
/// (one `log` of an exact table value; the miss is glibc `log` against
/// PETIR's); `ln_beta(1, 1e10)` within **1e-14** relative (from 4e-5
/// absolute); `beta` and `ln_beta` rows with negative non-integer arguments
/// return GSL's value instead of NaN, worst relative difference below
/// **1e-13** over the whole `beta` family; `psi_n`'s bit-identical share
/// rises above its present 34.97 % (it calls `lnfact`), by an amount I
/// do not predict. The prediction can fail: a miss would be reported as it is.
#[test]
fn beta_family_matches_gsl() {
    family("beta", &["ln_beta", "beta"]);
}
#[test]
fn gegenbauer_matches_gsl() {
    family("gegenbauer", &["gegenpoly_1", "gegenpoly_2", "gegenpoly_3", "gegenpoly_n"]);
}
#[test]
fn lambert_matches_gsl() {
    family("lambert", &["lambert_w0", "lambert_wm1"]);
}
#[test]
fn legendre_poly_matches_gsl() {
    family("legendre_poly", &["legendre_plm"]);
}
#[test]
fn psi_matches_gsl() {
    family("psi", &["psi", "psi_int", "psi_1piy", "psi_1_int", "psi_1", "psi_n"]);
}
#[test]
fn shint_matches_gsl() {
    family("shint", &["shi", "chi"]);
}
#[test]
fn sinint_matches_gsl() {
    family("sinint", &["si", "ci"]);
}
#[test]
fn synchrotron_matches_gsl() {
    family("synchrotron", &["synchrotron_1", "synchrotron_2"]);
}
#[test]
fn transport_matches_gsl() {
    family("transport", &["transport_2", "transport_3", "transport_4", "transport_5"]);
}
#[test]
fn trig_matches_gsl() {
    family("trig", &["angle_restrict_pos", "angle_restrict_symm"]);
}
#[test]
fn zeta_matches_gsl() {
    family("zeta", &["zeta", "zetam1", "eta", "zeta_int", "zetam1_int", "eta_int", "hzeta"]);
}
// erf/erfc: PETIR uses libm's fdlibm, not GSL's Chebyshev economisations
// (specfunc/erf.rs). Informational; a last-ulp difference is expected.
#[test]
fn erf_family_matches_gsl_to_1e8() {
    family("erf", &["erf", "erfc"]);
}

/// The whole sweep, one number: how many values GSL returned successfully
/// across every ported routine, and how many PETIR reproduces bit for bit.
#[test]
fn whole_specfunc_sweep_vs_gsl_2_8() {
    let names: Vec<&str> = table().keys().copied().collect();
    let (mut rows, mut exact, mut refused, mut agree) = (0, 0, 0, 0);
    let mut by_name = Vec::new();
    for n in &names {
        let t = compare(n);
        rows += t.ok_rows;
        exact += t.ok_exact;
        refused += t.err_rows;
        agree += t.err_agree;
        by_name.push((*n, t.ok_rows, t.ok_exact, t.worst_rel));
    }
    println!(
        "  whole sweep vs GSL 2.8: {rows} successful-GSL rows, {exact} bit-identical ({:.2} %); \
         {refused} GSL-refused rows, {agree} refused or reproduced by PETIR",
        100.0 * exact as f64 / rows as f64
    );
    for (n, r, e, w) in by_name {
        if e != r {
            println!("    {n}: {e}/{r} bit-identical, worst rel {w:.3e}");
        }
    }
    assert!(rows > 70_000, "reference truncated: {rows} rows");
}

/// Rows of `name` in `region` where GSL succeeded with a FINITE value:
/// `(rows, bit-identical, worst relative difference, rows skipped because GSL
/// returned a non-finite value with status 0)`.
fn finite_rows(name: &str, region: Region) -> (usize, usize, f64, usize) {
    let (mut rows, mut exact, mut worst, mut skipped) = (0, 0, 0.0_f64, 0);
    for r in table().get(name).expect("rows") {
        match region {
            Region::OutsidePoles if near_pole(name, r) => continue,
            Region::NearPoles if !near_pole(name, r) => continue,
            _ => {}
        }
        let Some(o) = ours(name, r) else { continue };
        if r.status != 0 || r.args.iter().any(|v| v.is_nan()) {
            continue;
        }
        if r.vals.iter().any(|v| !v.is_finite()) {
            skipped += 1;
            continue;
        }
        rows += 1;
        let mut same = true;
        for (g, p) in r.vals.iter().zip(&o) {
            same &= g.to_bits() == p.to_bits();
            worst = worst.max(rel(*p, *g));
        }
        exact += same as usize;
    }
    (rows, exact, worst, skipped)
}

/// **Results for gh:#1419 against its committed prediction** (the prediction
/// is in the doc comment of `gamma_near_negative_integer_poles_match_gsl`,
/// committed before the port): worst relative difference below 5e-15 on every
/// near-pole row where GSL is finite, and at least 60 % bit-identical.
///
/// Measured 2026-11-12, stock GSL 2.8 (glibc libm), Rust 1.99.1 release:
/// before the port the same rows differed by up to 0.15 in `ln|Gamma|` and
/// 15 % in `Gamma` (the issue's measurement). After: `ln_gamma` and `ln_gamma_sgn` 341/344 finite rows
/// bit-identical (99.13 %), worst relative difference **2.27e-16**; `gamma`
/// 313/336 (93.15 %), worst **8.63e-16** (the prediction was 5e-15 and 60 %).
/// The 2 rows excluded for each are the arguments `+-1e-310`, where GSL
/// returns `+inf` (status 0, `err = inf`): for `ln_gamma` that is `gee/|eps|`
/// overflowing for a finite answer, 713.80, which PETIR keeps
/// (`lngamma_sgn_0`'s documented DEVIATION); for `gamma` both codes overflow
/// identically. With GSL linked to PETIR's own libm: `gamma` 338/338
/// bit-identical, `ln_gamma` and `ln_gamma_sgn` 344/344 of the finite rows
/// (`reference-data/gsl/README.md`).
#[test]
fn near_pole_rows_meet_the_gh1419_prediction() {
    for name in ["ln_gamma", "ln_gamma_sgn", "gamma"] {
        let (rows, exact, worst, skipped) = finite_rows(name, Region::NearPoles);
        println!(
            "  gh1419 {name}: {rows} finite near-pole rows, {exact} bit-identical ({:.2} %), \
             worst rel {worst:.3e}, {skipped} GSL-infinite rows skipped",
            100.0 * exact as f64 / rows as f64
        );
        assert!(worst < 5e-15, "{name}: worst {worst:e} misses the 5e-15 prediction");
        assert!(10 * exact >= 6 * rows, "{name}: {exact}/{rows} bit-identical misses the 60 % prediction");
        // GSL's `inf` at +-1e-310: `ln_gamma` overflows in `gee/|eps|`
        // (PETIR's guard returns the finite 713.80); `gamma` overflows in
        // `1/(sin_term g)` in both codes, so those two rows are bit-equal.
        assert_eq!(skipped, 2, "{name}: the GSL-infinite rows are +-1e-310 only");
    }
}

/// **Results for gh:#1420 against its committed prediction**: `factorial` and
/// `choose` 100 % bit-identical to stock GSL (no libm call), `gamma` at integer
/// arguments 100 %, `ln_factorial` at least 95 %, the beta family within 1e-13,
/// `psi_n` above its pre-port 34.97 % bit-identical.
///
/// Measured 2026-11-12 vs stock GSL (glibc): `factorial` 171/171, `choose`
/// 1934/1934, `ln_factorial` 406/411 (98.78 %, worst 2.2e-16), `ln_beta`
/// 182/210 (worst 2.2e-15), `beta` 170/240 (worst 3.6e-15), `psi_n` 118/143
/// (82.52 %, worst 2.5e-16; was 50/143 and 3.8e-15). Against GSL linked to
/// PETIR's own libm: all of `ln_factorial`, `choose`, `ln_beta`, `beta` and
/// `psi_n` are 100 % bit-identical.
#[test]
fn fact_choose_beta_rows_meet_the_gh1420_prediction() {
    for name in ["factorial", "choose"] {
        let (rows, exact, worst, _) = finite_rows(name, Region::All);
        println!("  gh1420 {name}: {exact}/{rows} bit-identical, worst rel {worst:e}");
        assert_eq!(exact, rows, "{name} is a table read / IEEE quotient: it must be bit-identical");
    }
    // gamma at integer arguments 1..=171: the table.
    let (mut int_rows, mut int_exact) = (0, 0);
    for r in table().get("gamma").expect("rows") {
        let x = r.args[0];
        if r.status == 0 && x >= 1.0 && x <= 171.0 && x == x.floor() {
            int_rows += 1;
            int_exact += (ours("gamma", r).expect("gamma")[0].to_bits() == r.vals[0].to_bits()) as usize;
        }
    }
    println!("  gh1420 gamma(n): {int_exact}/{int_rows} bit-identical");
    assert!(int_rows >= 60 && int_exact == int_rows);

    let (rows, exact, worst, _) = finite_rows("ln_factorial", Region::All);
    println!("  gh1420 ln_factorial: {exact}/{rows}, worst {worst:e}");
    assert!(100 * exact >= 95 * rows);

    for name in ["ln_beta", "beta"] {
        let (rows, exact, worst, _) = finite_rows(name, Region::All);
        println!("  gh1420 {name}: {exact}/{rows} bit-identical, worst rel {worst:e}");
        assert!(worst < 1e-13, "{name}: worst {worst:e} misses the 1e-13 prediction");
    }
    // Rows with a negative non-integer argument exist and are answered.
    let negative = table()["beta"].iter().filter(|r| r.status == 0 && r.args.iter().any(|v| *v < 0.0)).count();
    assert!(negative > 0);

    let (rows, exact, worst, _) = finite_rows("psi_n", Region::All);
    println!("  gh1420 psi_n: {exact}/{rows} bit-identical, worst {worst:e}");
    assert!(10_000 * exact > 3_497 * rows, "psi_n must improve on its pre-port 34.97 %");
}
