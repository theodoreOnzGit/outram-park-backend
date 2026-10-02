// SPDX-License-Identifier: GPL-3.0
//
// Shared harness for the FLEXPART code-to-code tests (`tests/flexpart_*`).
// Each test file includes it with `mod common;`.

//! Fixture parsing and the two-precision comparison used by every FLEXPART
//! code-to-code test.
//!
//! # Fixture format
//!
//! `function,arg1;arg2;...,out1;out2;...`, one row per upstream call, written
//! by a Fortran driver under `dev/`. Rows whose function is a setup name echo a
//! synthetic input instead of a call.
//!
//! The real(4) fixture prints 9 significant digits, which identify an `f32`
//! exactly, so it is parsed **as `f32` and widened**: the port is fed the very
//! values the Fortran was fed. Functions listed as double precision (Julian
//! dates are `real(kind=dp)` at both precisions upstream) are parsed as `f64`.
//!
//! # The comparison
//!
//! Every group is checked against **both** builds. Against real(8) the bound
//! measures the translation; against real(4) it measures FLEXPART as shipped.
//! See [`Real4Rule`] for the one, opt-in, way a real(4) residual may be
//! attributed to upstream's own single precision.

#![allow(dead_code)]

/// Which upstream build a fixture came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Precision {
    /// As FLEXPART ships: default `real` is `real(4)`.
    Real4,
    /// `-fdefault-real-8 -fdefault-double-8`.
    Real8,
}

impl Precision {
    /// `"real4"` / `"real8"`.
    pub fn label(self) -> &'static str {
        match self {
            Precision::Real4 => "real4",
            Precision::Real8 => "real8",
        }
    }

    /// A driver literal as this build stored it.
    pub fn lit(self, x: f64) -> f64 {
        match self {
            Precision::Real4 => x as f32 as f64,
            Precision::Real8 => x,
        }
    }
}

/// One fixture row.
#[derive(Clone, Debug)]
pub struct Row {
    /// Upstream routine (or setup-row name).
    pub function: String,
    /// Arguments, as the build stored them.
    pub args: Vec<f64>,
    /// Outputs, as the build stored them.
    pub outs: Vec<f64>,
}

fn parse_num(s: &str, p: Precision, dp: bool) -> f64 {
    let s = s.trim();
    if dp || p == Precision::Real8 {
        s.parse::<f64>().unwrap_or_else(|_| panic!("f64 `{s}`"))
    } else {
        s.parse::<f32>().unwrap_or_else(|_| panic!("f32 `{s}`")) as f64
    }
}

/// Parse a fixture. `dp_functions` are parsed as `f64` at both precisions.
pub fn parse(fixture: &str, p: Precision, dp_functions: &[&str]) -> Vec<Row> {
    fixture
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("function,"))
        .map(|l| {
            let mut f = l.splitn(3, ',');
            let function = f.next().expect("function").trim().to_string();
            let dp = dp_functions.contains(&function.as_str());
            let args = f
                .next()
                .expect("args")
                .split(';')
                .map(|s| parse_num(s, p, dp))
                .collect();
            let outs = f
                .next()
                .unwrap_or("")
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| parse_num(s, p, dp))
                .collect();
            Row {
                function,
                args,
                outs,
            }
        })
        .collect()
}

/// Both fixtures of one driver, parsed.
pub struct Fixtures {
    /// The shipped build.
    pub real4: Vec<Row>,
    /// The real(8) build.
    pub real8: Vec<Row>,
}

impl Fixtures {
    /// Parse both.
    pub fn new(real4: &str, real8: &str, dp_functions: &[&str]) -> Self {
        Self {
            real4: parse(real4, Precision::Real4, dp_functions),
            real8: parse(real8, Precision::Real8, dp_functions),
        }
    }

    /// The rows of one build.
    pub fn rows(&self, p: Precision) -> &[Row] {
        match p {
            Precision::Real4 => &self.real4,
            Precision::Real8 => &self.real8,
        }
    }
}

/// `true` when `got` matches `want` within a relative `tol` OR an absolute
/// `floor`. A NaN agrees only with a NaN.
pub fn agrees(got: f64, want: f64, tol: f64, floor: f64) -> bool {
    if want.is_nan() || got.is_nan() {
        return want.is_nan() && got.is_nan();
    }
    if got == want {
        return true;
    }
    let abs = (got - want).abs();
    abs <= floor || abs / want.abs() <= tol
}

/// How the real(4) fixture is judged for one group.
#[derive(Clone, Copy, Debug)]
pub enum Real4Rule {
    /// Plain `tol4` / `floor4` bound on every output.
    Strict,
    /// As `Strict`, OR the port lies within [`SPREAD_FACTOR`] x upstream's own
    /// real(4)-vs-real(8) distance for that output. Opt-in per group, only
    /// where the group's doc shows the routine is ill-conditioned in `f32`. It
    /// states that the shipped-build residual is FLEXPART's single precision,
    /// which the real(8) check then proves is not a translation error. Outputs
    /// accepted this way are counted and printed, never hidden.
    PrecisionSpread,
    /// As [`Real4Rule::PrecisionSpread`], restricted to the listed output
    /// columns (0-based); every other output is held strictly.
    PrecisionSpreadOn(&'static [usize]),
}

/// See [`Real4Rule::PrecisionSpread`]. A factor, not a tuning knob: the two
/// builds see inputs that differ by `f32` rounding, so their distance is an
/// order-of-magnitude scale for single-precision error, nothing finer.
pub const SPREAD_FACTOR: f64 = 4.0;

/// Tolerances for one group.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    /// Relative bound against real(8) (the translation).
    pub tol8: f64,
    /// Absolute floor against real(8).
    pub floor8: f64,
    /// Relative bound against real(4) (upstream's precision).
    pub tol4: f64,
    /// Absolute floor against real(4).
    pub floor4: f64,
    /// How real(4) misses may be attributed.
    pub rule: Real4Rule,
}

impl Bounds {
    /// Relative bounds only, strict real(4).
    pub const fn rel(tol8: f64, tol4: f64) -> Self {
        Self {
            tol8,
            floor8: 0.0,
            tol4,
            floor4: 0.0,
            rule: Real4Rule::Strict,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_one<S, E>(
    fx: &Fixtures,
    p: Precision,
    name: &str,
    tol: f64,
    floor: f64,
    rule: Real4Rule,
    in_scope: &S,
    evaluate: &E,
) -> f64
where
    S: Fn(&Row, Precision) -> bool,
    E: Fn(&Row, Precision) -> Option<Vec<f64>>,
{
    let label = p.label();
    let group: Vec<&Row> = fx
        .rows(p)
        .iter()
        .filter(|r| r.function == name && in_scope(r, p))
        .collect();
    let group8: Vec<&Row> = fx
        .rows(Precision::Real8)
        .iter()
        .filter(|r| r.function == name && in_scope(r, p))
        .collect();
    assert!(
        !group.is_empty(),
        "no `{name}` rows in the {label} fixture — regenerate with dev/build_reference.sh"
    );
    let mut worst = 0.0_f64;
    let mut worst_at = String::new();
    let mut failures = Vec::new();
    let mut spread_explained = 0usize;
    let mut exact = 0usize;
    let mut n_out = 0usize;
    for (i, r) in group.iter().enumerate() {
        let got = evaluate(r, p).unwrap_or_else(|| panic!("no evaluator wired for `{name}`"));
        assert_eq!(got.len(), r.outs.len(), "{name}: output count");
        for (k, (&g, &w)) in got.iter().zip(&r.outs).enumerate() {
            n_out += 1;
            if g == w || (g.is_nan() && w.is_nan()) {
                exact += 1;
            }
            let mut ok = agrees(g, w, tol, floor);
            let spread_applies = match rule {
                Real4Rule::Strict => false,
                Real4Rule::PrecisionSpread => true,
                Real4Rule::PrecisionSpreadOn(cols) => cols.contains(&k),
            };
            if !ok && spread_applies && p == Precision::Real4 {
                let r8 = group8.get(i).expect("real8 row aligned with real4 row");
                let spread = (r8.outs[k] - w).abs();
                if (g - w).abs() <= SPREAD_FACTOR * spread {
                    ok = true;
                    spread_explained += 1;
                }
            }
            if !ok {
                failures.push(format!(
                    "{name}{:?} out[{k}]: port {g:e} vs FLEXPART {w:e}",
                    r.args
                ));
            }
            if g.is_finite() && w.is_finite() && w != 0.0 && (g - w).abs() > floor {
                let d = (g - w).abs() / w.abs();
                if d > worst {
                    worst = d;
                    worst_at = format!("{:?} out[{k}]", r.args);
                }
            }
        }
    }
    let spread_note = if spread_explained > 0 {
        format!(", {spread_explained} outputs explained by upstream's real4-vs-real8 spread")
    } else {
        String::new()
    };
    println!(
        "{label} {name:<26} {:>4} rows, {exact}/{n_out} outputs bit-exact, max_rel_dev = {worst:.3e} (tol {tol:e}, floor {floor:e}){spread_note} {worst_at}",
        group.len()
    );
    assert!(
        failures.is_empty(),
        "{label}/{name}: {} disagreements (tol {tol:e}, floor {floor:e}); first:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    worst
}

/// Check group `name` against both builds. `in_scope` filters rows (state the
/// reason for every exclusion where it is decided); `evaluate` drives the port
/// for one row and returns its outputs in fixture order (`None` = not wired,
/// which fails).
pub fn check_group<S, E>(fx: &Fixtures, name: &str, b: Bounds, in_scope: S, evaluate: E)
where
    S: Fn(&Row, Precision) -> bool,
    E: Fn(&Row, Precision) -> Option<Vec<f64>>,
{
    check_one(
        fx,
        Precision::Real8,
        name,
        b.tol8,
        b.floor8,
        Real4Rule::Strict,
        &in_scope,
        &evaluate,
    );
    check_one(
        fx,
        Precision::Real4,
        name,
        b.tol4,
        b.floor4,
        b.rule,
        &in_scope,
        &evaluate,
    );
}
