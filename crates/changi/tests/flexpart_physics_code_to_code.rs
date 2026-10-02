// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart`'s stage-1 physics against
// upstream FLEXPART v10.4 (commit 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage-1 physics (turbulence, CBL, deposition, PBL, solar), code-to-code
//!
//! ## Methodology
//!
//! Every reference value is produced by **compiling and running upstream
//! FLEXPART itself**. `dev/flexpart_reference_physics.f90` links the upstream
//! routines verbatim (`hanna*.f90`, `cbl.f90`, `getrb.f90`, `getrc.f90`,
//! `partdep.f90`, `getvdep.f90`, `get_settling.f90`, `pbl_profile.f90`,
//! `richardson.f90`, `qvsat.f90`, `windalign.f90`, `juldate.f90`,
//! `caldate.f90`, `zenithangle.f90`, `photo_O1D.f90`, `distance*.f90`, plus
//! the modules `par_mod`, `com_mod` and `hanna_mod` they read). The routines
//! that read meteorology from `com_mod` are driven on **synthetic fields the
//! driver writes into `com_mod` directly**, so no GRIB or NetCDF file is
//! involved. Each routine is swept over a grid chosen to straddle its branches
//! and guards.
//!
//! The driver is built twice, by `dev/build_reference.sh`:
//!
//! - `real(4)`, as FLEXPART ships — the residual measures upstream's precision;
//! - `-fdefault-real-8 -fdefault-double-8` — the residual measures the
//!   **translation**, and must sit near machine precision.
//!
//! Fixture rows are `function,args,outputs`, `;`-separated. The real4 fixture
//! prints 9 significant digits, which round-trips a `f32` exactly, so its
//! arguments are parsed **as `f32` then widened**: the port is fed the very
//! values the Fortran was fed, not their decimal approximations.
//!
//! Some rows are setup, not calls (`particle_bins`, `wesely`,
//! `richardson.column`): they record the synthetic inputs the driver put in
//! `com_mod`, so the port is fed the same tables.
//!
//! A NaN output agrees only with a NaN reference: the routines that can
//! produce one upstream (degenerate `richardson` profiles) are expected to
//! produce one here.
//!
//! ## Results
//!
//! Re-print with `cargo test --release -p changi --test
//! flexpart_physics_code_to_code -- --nocapture`. The measured table and its
//! interpretation are in `docs/flexpart-code-to-code.md`.
//!
//! ## What this does NOT establish
//!
//! **Verification, not validation.** It shows the Rust computes what the
//! Fortran computes, on synthetic inputs. It says nothing about whether
//! FLEXPART's parameterisations reproduce measured dispersion. Per the crate
//! scope limit, nothing here supports emergency response or dose assessment
//! for real populations.

use changi::flexpart::boundary_layer::{f_qvsat, pbl_profile, richardson};
use changi::flexpart::calendar::{caldate, juldate};
use changi::flexpart::geodesy::{distance, distance2};
use changi::flexpart::solar::{photo_o1d, zenithangle};
use changi::flexpart::surface_layer::MetDataFormat;
use changi::flexpart::aerosol::AerosolBins;
use changi::flexpart::cbl::cbl;
use changi::flexpart::constants::{NI, NUMCLASS};
use changi::flexpart::dry_deposition::{
    get_settling, getrb, getrc, getvdep, partdep, DepositionSpecies, GasSpecies, SurfaceMet,
    SurfaceResistances, NSEASON,
};
use changi::flexpart::turbulence::{hanna, hanna1, hanna_short, windalign, HannaState};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_physics_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_physics_real8.csv");

#[derive(Clone, Copy, PartialEq)]
enum Precision {
    Real4,
    Real8,
}

struct Row {
    function: String,
    args: Vec<f64>,
    outs: Vec<f64>,
}

fn parse_num(s: &str, p: Precision) -> f64 {
    let s = s.trim();
    match p {
        // f32 first: the printed 9 digits identify the stored f32 exactly.
        Precision::Real4 => s.parse::<f32>().unwrap_or_else(|_| panic!("f32 `{s}`")) as f64,
        Precision::Real8 => s.parse::<f64>().unwrap_or_else(|_| panic!("f64 `{s}`")),
    }
}

/// Like [`parse_num`] but always `f64`: for the Julian dates, which are
/// `real(kind=dp)` in upstream at both precisions.
fn parse_f64(s: &str) -> f64 {
    s.trim()
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("f64 `{s}`"))
}

fn parse(fixture: &str, p: Precision) -> Vec<Row> {
    fixture
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("function,"))
        .map(|l| {
            let mut f = l.splitn(3, ',');
            let function = f.next().expect("function").trim().to_string();
            // Calendar and zenith rows are written in double precision at both
            // precisions (`rowvd` in the driver).
            let dp = matches!(function.as_str(), "juldate" | "caldate" | "zenithangle");
            let num = |s: &str| if dp { parse_f64(s) } else { parse_num(s, p) };
            let args = f.next().expect("args").split(';').map(num).collect();
            let outs = f
                .next()
                .unwrap_or("")
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(num)
                .collect();
            Row {
                function,
                args,
                outs,
            }
        })
        .collect()
}

/// Synthetic `com_mod` tables recorded by the driver's setup rows, plus the
/// precision the driver ran at (for the few literals the driver uses that are
/// not echoed in a row).
struct Setup {
    p: Precision,
    /// `particle_bins` rows: (scale, bins).
    bins: Vec<(f64, AerosolBins)>,
    /// `wesely` rows: resistances `[season][class]`.
    wesely: [[SurfaceResistances; NUMCLASS]; NSEASON],
    z0: [f64; NUMCLASS],
    landuse: [f64; NUMCLASS],
    /// `richardson.column` rows: (format, profile id, akz, bkz, tt, qv, u, v).
    columns: Vec<RichardsonColumn>,
}

struct RichardsonColumn {
    format: f64,
    profile: f64,
    levels: [Vec<f64>; 6],
}

impl Setup {
    /// A driver literal as that build stored it.
    fn lit(&self, x: f64) -> f64 {
        match self.p {
            Precision::Real4 => x as f32 as f64,
            Precision::Real8 => x,
        }
    }

    fn bins_for(&self, scale: f64) -> AerosolBins {
        self.bins
            .iter()
            .find(|(s, _)| *s == scale)
            .unwrap_or_else(|| panic!("no particle_bins row for scale {scale}"))
            .1
    }
}

fn build_setup(rows: &[Row], p: Precision) -> Setup {
    let mut s = Setup {
        p,
        bins: Vec::new(),
        wesely: [[SurfaceResistances::default(); NUMCLASS]; NSEASON],
        z0: [0.0; NUMCLASS],
        landuse: [0.0; NUMCLASS],
        columns: Vec::new(),
    };
    for r in rows {
        match r.function.as_str() {
            "particle_bins" => {
                let a = &r.args;
                let mut b = AerosolBins {
                    mass_fraction: [0.0; NI],
                    schmidt_factor: [0.0; NI],
                    settling_velocity: [0.0; NI],
                    cunningham: [0.0; NI],
                };
                for j in 0..NI {
                    b.settling_velocity[j] = a[1 + j];
                    b.schmidt_factor[j] = a[1 + NI + j];
                    b.mass_fraction[j] = a[1 + 2 * NI + j];
                }
                s.bins.push((a[0], b));
            }
            "wesely" => {
                let a = &r.args;
                let (i, j) = (a[0] as usize - 1, a[1] as usize - 1);
                s.wesely[i][j] = SurfaceResistances {
                    ri: a[2],
                    rac: a[3],
                    rcl: a[4],
                    rgs: a[5],
                    rlu: a[6],
                };
                s.z0[j] = a[7];
                s.landuse[j] = a[8];
            }
            "richardson.column" => {
                let a = &r.args;
                let n = (a.len() - 2) / 6;
                let lv = |c: usize| a[2 + c * n..2 + (c + 1) * n].to_vec();
                s.columns.push(RichardsonColumn {
                    format: a[0],
                    profile: a[1],
                    levels: [lv(0), lv(1), lv(2), lv(3), lv(4), lv(5)],
                });
            }
            _ => {}
        }
    }
    s
}

/// `None` (upstream leaves the output unassigned) is the driver's sentinel.
fn or_sentinel(v: Option<f64>, sentinel: f64) -> f64 {
    v.unwrap_or(sentinel)
}

fn hanna_state(a: &[f64]) -> (HannaState, f64) {
    // args: ust, wst, ol, h, z, prior sigw, prior dsigw2dz, prior tlu, prior tlv
    // The other priors are fixed sentinels in the driver.
    let z = a[4];
    let s = HannaState {
        ust: a[0],
        wst: a[1],
        ol: a[2],
        h: a[3],
        zeta: z / a[3],
        sigu: -1.0,
        sigv: -2.0,
        sigw: a[5],
        dsigwdz: 0.0,
        dsigw2dz: a[6],
        tlu: a[7],
        tlv: a[8],
        tlw: 7.0,
    };
    (s, z)
}

fn hanna_outs(s: &HannaState, gradient: f64) -> Vec<f64> {
    vec![s.ust, s.sigu, s.sigv, s.sigw, gradient, s.tlu, s.tlv, s.tlw]
}

/// Drive the port for one fixture row; `None` = no evaluator wired (a failure,
/// so an unported group can never pass silently).
fn evaluate(setup: &Setup, name: &str, a: &[f64]) -> Option<Vec<f64>> {
    Some(match name {
        "hanna" => {
            let (mut s, z) = hanna_state(a);
            hanna(&mut s, z);
            hanna_outs(&s, s.dsigwdz)
        }
        "hanna1" => {
            let (mut s, z) = hanna_state(a);
            hanna1(&mut s, z);
            hanna_outs(&s, s.dsigw2dz)
        }
        "hanna_short" => {
            let (mut s, z) = hanna_state(a);
            hanna_short(&mut s, z);
            hanna_outs(&s, s.dsigwdz)
        }
        "windalign" => {
            let (u, v) = windalign(a[0], a[1], a[2], a[3]);
            vec![u, v]
        }
        "cbl" => {
            let t = cbl(
                a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11],
            );
            // flagrein enters as 0 in the driver and is only ever set to 1.
            vec![
                t.ptot,
                t.q,
                t.phi,
                t.ath,
                t.bth,
                if t.reinitialise { 1.0 } else { 0.0 },
            ]
        }
        "getrb" => vec![or_sentinel(getrb(a[0], a[1], a[2], a[3]), -7.0)],
        "getrc" => {
            let cell = SurfaceResistances {
                ri: a[4],
                rac: a[5],
                rcl: a[6],
                rgs: a[7],
                rlu: a[8],
            };
            let sp = GasSpecies {
                rm: a[9],
                henry: a[10],
                f0: a[11],
                reldiff: a[12],
            };
            vec![or_sentinel(getrc(&cell, &sp, a[0], a[1], a[2], a[3]), -7.0)]
        }
        "partdep" => {
            let bins = setup.bins_for(a[3]);
            vec![partdep(a[4], a[5], &bins, a[1], a[0], a[2])]
        }
        "getvdep" => {
            let jul = 2_459_000.0 + a[0];
            // ylat = jy*dy + ylat0 with dy = 1
            let ylat = a[11] * 1.0 + a[12];
            let met = SurfaceMet {
                ust: a[2],
                temp: a[3],
                pa: a[4],
                ol: a[5],
                gr: a[6],
                rh: a[7],
                rr: a[8],
                snow: a[9],
            };
            let species = match a[10] as i32 {
                1 => DepositionSpecies::Gas(GasSpecies {
                    reldiff: setup.lit(1.6),
                    henry: setup.lit(1.0e-2),
                    f0: 1.0,
                    rm: 0.0,
                }),
                2 => DepositionSpecies::Particle {
                    density: 2300.0,
                    bins: setup.bins_for(1.0),
                },
                _ => DepositionSpecies::Prescribed(setup.lit(4.0e-3)),
            };
            vec![getvdep(
                jul,
                ylat,
                &met,
                &setup.landuse,
                &setup.z0,
                &setup.wesely,
                &species,
            )]
        }
        "get_settling" => {
            let height = [0.0, 40.0, 150.0, 500.0, 1500.0, 4000.0];
            let (mut tt, mut rho) = ([0.0; 6], [0.0; 6]);
            for k in 0..6 {
                // the driver's synthetic column, at its own precision
                match setup.p {
                    Precision::Real4 => {
                        let h = height[k] as f32;
                        tt[k] = (293.0_f32 - 0.0065_f32 * h) as f64;
                        rho[k] = (1.2_f32 * (-h / 8500.0_f32).exp()) as f64;
                    }
                    Precision::Real8 => {
                        tt[k] = 293.0 - 0.0065 * height[k];
                        rho[k] = 1.2 * (-height[k] / 8500.0).exp();
                    }
                }
            }
            let v = get_settling(a[0], &height, &tt, &rho, a[1], a[2], a[3], a[4]);
            vec![v.expect("zt below the column top")]
        }
        "pbl_profile" => {
            let r = pbl_profile(a[0], a[1], a[2], a[3], a[4], a[5], a[6]);
            vec![r.stress, r.hf]
        }
        "qvsat" => vec![f_qvsat(a[0], a[1])],
        "richardson" => {
            let c = setup
                .columns
                .iter()
                .find(|c| c.format == a[0] && c.profile == a[1])
                .expect("richardson.column row");
            // class_gribfile: NCEP = 1, ECMWF = 2
            let format = if a[0] == 2.0 {
                MetDataFormat::Ecmwf {
                    akm: [0.0; 2],
                    bkm: [0.0; 2],
                }
            } else {
                MetDataFormat::Ncep
            };
            let [akz, bkz, tt, qv, u, v] = &c.levels;
            let m = richardson(
                a[2], a[3], akz, bkz, tt, qv, u, v, a[4], a[5], a[6], &format,
            );
            vec![m.h, m.wst, m.hmixplus]
        }
        "zenithangle" => vec![zenithangle(a[0], a[1], a[2])],
        "photo_O1D" => vec![photo_o1d(a[0]).expect("sza >= 0")],
        "distance" => vec![distance(a[0], a[1], a[2], a[3])],
        "distance2" => vec![distance2(a[0], a[1], a[2], a[3])],
        "juldate" => vec![juldate(a[0] as i64, a[1] as i64)],
        "caldate" => {
            let (ymd, hms) = caldate(a[0]);
            vec![ymd as f64, hms as f64]
        }
        _ => return None,
    })
}

/// Rows a group skips, with the reason stated where it is decided.
fn in_scope(name: &str, a: &[f64], p: Precision) -> bool {
    match name {
        // The port replaces Numerical Recipes' caldate (Julian/Gregorian switch)
        // with a proleptic-Gregorian algorithm, valid from 1582-10-15
        // (JD 2299161). Earlier dates are out of the documented range.
        //
        // And the shipped real(4) caldate returns 16010231 — not a date — for
        // 1600-02-29 (JD 2305507), because its `(julday-1867216)-0.25)/36524.25`
        // and `6680.+((jb-2439870)-122.1)/365.25` are evaluated in default
        // `real`. That is an upstream single-precision defect; the real8 build
        // gets 16000229 and so does the port. Pinned separately by
        // `caldate_real4_century_leap_day_defect_is_upstreams`.
        "caldate" => a[0] >= 2_299_161.0 && !(p == Precision::Real4 && a[0].floor() == 2_305_507.0),
        // The f32 image of 253.15 K is qvsat's liquid/ice switch itself; see
        // `qvsat_matches_flexpart`.
        "qvsat" => !(p == Precision::Real4 && a[1] == 253.15_f32 as f64),
        _ => true,
    }
}

fn agrees(got: f64, want: f64, tol: f64, floor: f64) -> bool {
    if want.is_nan() || got.is_nan() {
        return want.is_nan() && got.is_nan();
    }
    if got == want {
        return true;
    }
    let abs = (got - want).abs();
    abs <= floor || abs / want.abs() <= tol
}

/// How the real4 fixture is judged for one group.
#[derive(Clone, Copy)]
enum Real4Rule {
    /// Plain `tol4` / `floor4` bound on every output.
    Strict,
    /// As `Strict`, OR the port lies within `SPREAD_FACTOR` x upstream's own
    /// real4-vs-real8 distance for that output. Opt-in per group, only where
    /// the group's doc shows the routine is ill-conditioned in `f32`: it states
    /// that the shipped-build residual is FLEXPART's single precision, which
    /// the real8 check then proves is not a translation error. Rows explained
    /// this way are counted and printed, never hidden.
    PrecisionSpread,
}

/// See [`Real4Rule::PrecisionSpread`]. A factor, not a tuning knob: the two
/// builds see inputs that differ by f32 rounding, so their distance is an
/// order-of-magnitude scale for single-precision error, nothing finer.
const SPREAD_FACTOR: f64 = 4.0;

/// Check one group against one fixture; returns the worst relative deviation
/// among outputs outside the absolute floor.
///
/// `reference8` is the real8 parse of the same driver, used only by
/// [`Real4Rule::PrecisionSpread`] (rows align because both builds run the same
/// driver over the same loops).
#[allow(clippy::too_many_arguments)]
fn check(
    fixture: &str,
    p: Precision,
    name: &str,
    tol: f64,
    floor: f64,
    rule: Real4Rule,
    reference8: &[Row],
) -> f64 {
    let label = if p == Precision::Real4 {
        "real4"
    } else {
        "real8"
    };
    let rows = parse(fixture, p);
    let setup = build_setup(&rows, p);
    let group: Vec<&Row> = rows
        .iter()
        .filter(|r| r.function == name && in_scope(name, &r.args, p))
        .collect();
    let group8: Vec<&Row> = reference8
        .iter()
        .filter(|r| r.function == name && in_scope(name, &r.args, p))
        .collect();
    assert!(
        !group.is_empty(),
        "no `{name}` rows in the {label} fixture — regenerate with dev/build_reference.sh"
    );
    let mut worst = 0.0_f64;
    let mut worst_at = String::new();
    let mut failures = Vec::new();
    let mut spread_explained = 0usize;
    for (i, r) in group.iter().enumerate() {
        let got = evaluate(&setup, name, &r.args)
            .unwrap_or_else(|| panic!("no evaluator wired for `{name}`"));
        assert_eq!(got.len(), r.outs.len(), "{name}: output count");
        for (k, (&g, &w)) in got.iter().zip(&r.outs).enumerate() {
            let mut ok = agrees(g, w, tol, floor);
            if !ok && matches!(rule, Real4Rule::PrecisionSpread) && p == Precision::Real4 {
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
        "{label} {name:<14} {:>4} rows, max_rel_dev = {worst:.3e} (tol {tol:e}, floor {floor:e}){spread_note} {worst_at}",
        group.len()
    );
    assert!(
        failures.is_empty(),
        "{label}/{name}: {} disagreements (tol {tol:e}, floor {floor:e}); first: \n{}",
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

fn check_both(name: &str, tol8: f64, floor8: f64, tol4: f64, floor4: f64) {
    check_both_rule(name, tol8, floor8, tol4, floor4, Real4Rule::Strict);
}

fn check_both_rule(name: &str, tol8: f64, floor8: f64, tol4: f64, floor4: f64, rule: Real4Rule) {
    let r8 = parse(FIXTURE_REAL8, Precision::Real8);
    check(
        FIXTURE_REAL8,
        Precision::Real8,
        name,
        tol8,
        floor8,
        Real4Rule::Strict,
        &r8,
    );
    check(
        FIXTURE_REAL4,
        Precision::Real4,
        name,
        tol4,
        floor4,
        rule,
        &r8,
    );
}

/// Upstream's `1e-10` substitute for a zero gradient, as the shipped build
/// stores it (`1.0000000133514320e-10`); see [`hanna_matches_flexpart`].
const UNDERFLOW_SUBSTITUTE_F32: f64 = 1e-10_f32 as f64;

// ── turbulence ────────────────────────────────────────────────────────────────

/// `hanna.f90` over neutral, unstable and stable layers, z from 0.175 m to
/// above h. All eight outputs, including the carried-over state.
///
/// # The real4 absolute floor, `1e-10`
///
/// In a stable layer with `u* = 1e-6 m/s` the gradient `d sigma_w / dz` is
/// `~1e-48` or smaller: below `f32`'s smallest subnormal, so the shipped build
/// rounds it to `0` and then applies upstream's own `if (dsigwdz == 0)
/// dsigwdz = 1e-10` substitution, while `f64` keeps the tiny value. The real8
/// build agrees with the port **exactly** on every one of those rows, which is
/// the statement that the translation is right. The floor equals upstream's
/// substitute value, and is `1e-8` relative on the smallest `sigma` in the
/// sweep, so it cannot hide a real error in any other output.
#[test]
fn hanna_matches_flexpart() {
    check_both("hanna", 1e-13, 0.0, 1e-5, UNDERFLOW_SUBSTITUTE_F32);
}

/// `hanna1.f90`, including the unassigned `sigw`/`dsigw2dz` path for unstable
/// `zeta >= 1`, which must return the prior state.
#[test]
fn hanna1_matches_flexpart() {
    // Same f32-underflow floor as `hanna` (there the gradient flushes to -0).
    check_both("hanna1", 1e-13, 0.0, 1e-5, UNDERFLOW_SUBSTITUTE_F32);
}

/// `hanna_short.f90`, including `max(10, tlu)` applied to the stale `tlu`.
#[test]
fn hanna_short_matches_flexpart() {
    // Same f32-underflow floor as `hanna`.
    check_both("hanna_short", 1e-13, 0.0, 1e-5, UNDERFLOW_SUBSTITUTE_F32);
}

/// `windalign.f90` over all four quadrants and the calm case.
#[test]
fn windalign_matches_flexpart() {
    check_both("windalign", 1e-14, 0.0, 1e-6, 0.0);
}

// ── convective boundary layer ───────────────────────────────────────────────

/// `cbl.f90` over velocity, height, `w*`, `L`, `sigma_w` and both time
/// directions, including `w* = 0` (zero skewness branch) and the
/// `-h/L < 15` transition taper.
///
/// # Tolerances, and a finding about upstream's single precision
///
/// Against real8 the bound is `1e-12`. The measured maximum is `2.4e-15`, and
/// the residual is the `erf` substitution (petir vs the gfortran intrinsic).
///
/// Against the shipped real4 build, the flux term `Phi` and the drift `a`
/// **cannot** be held to any relative bound. Where the particle velocity
/// sits far in the tail of both Gaussian modes, the PDF `ptot` falls to
/// `1e-12 .. 1e-7`. `Phi` is then a sum of `O(1e-4)` terms that cancels to
/// `O(ptot)`, and `a = (...)/ptot` divides that noise by a tiny number.
/// Measured from the fixtures alone, upstream's real4 build differs from its
/// own real8 build by up to `1.1e3` relative in `Phi` and `5.1e3` in `a`
/// (the drift's sign flips in some rows), over 70 of 900 rows. That is a
/// property of FLEXPART as shipped, not of this port.
///
/// So the real4 group uses [`Real4Rule::PrecisionSpread`]. The real8 check
/// above is what shows the translation is right.
#[test]
fn cbl_matches_flexpart() {
    check_both_rule("cbl", 1e-12, 0.0, 1e-4, 0.0, Real4Rule::PrecisionSpread);
}

// ── dry deposition ────────────────────────────────────────────────────────────

/// `getrb.f90`, including `reldiff <= 0` (rb left unassigned; sentinel `-7`).
#[test]
fn getrb_matches_flexpart() {
    check_both("getrb", 1e-14, 0.0, 1e-6, 0.0);
}

/// `getrc.f90` over temperatures either side of the 0–40 °C stomatal window,
/// dark and bright, dry and humid, dry and wet, five resistance sets including
/// the `9999`/`1e25` markers and a zero, three gases, and `reldiff < 0`.
#[test]
fn getrc_matches_flexpart() {
    check_both("getrc", 1e-14, 0.0, 1e-5, 0.0);
}

/// `partdep.f90` over `u*` either side of the `1e-5` cutoff, three `r_a`, three
/// bin scales (so `alpha` lands either side of `log10(eps)`), and a
/// non-particle density.
#[test]
fn partdep_matches_flexpart() {
    check_both("partdep", 1e-14, 0.0, 1e-5, 0.0);
}

/// `getvdep.f90` composed end to end (season from `caldate`, `getrb`, `raerod`
/// per landuse class, `getrc`, `partdep`, the `dryvel` override) for a gas, a
/// particle and a prescribed-velocity species, at 52°N, 4°N (always summer)
/// and 37°S (the 182-day shift), with and without snow cover.
#[test]
fn getvdep_matches_flexpart() {
    check_both("getvdep", 1e-13, 0.0, 1e-5, 0.0);
}

/// `get_settling.f90` over five heights in a six-level column and five
/// diameters spanning the three drag regimes.
#[test]
fn get_settling_matches_flexpart() {
    check_both("get_settling", 1e-13, 0.0, 1e-5, 0.0);
}

// ── boundary layer ────────────────────────────────────────────────────────────

/// `pbl_profile.f90` across all four branches: no shear, neutral, stable
/// non-converging (`L = 50`) and the iteration, at two model-level heights.
#[test]
fn pbl_profile_matches_flexpart() {
    check_both("pbl_profile", 1e-13, 0.0, 1e-5, 0.0);
}

/// `f_qvsat` either side of the 253.15 K liquid/ice switch.
///
/// # Three real4 rows are out of scope, and why
///
/// The sweep puts `t = 253.149 K` and `t = 253.15 K` either side of the
/// switch. In `f32` the second is `253.14999389648438`, the same number as the
/// `f32` image of the literal `253.15`. So the shipped build sees
/// `t == threshold` and takes the liquid branch. The port, fed that same `f32`
/// value, compares it with an `f64` threshold, finds it below, and takes the
/// ice branch. The two formulas differ by 18 % there.
///
/// That is a branch decided by single-precision rounding of the threshold, not
/// a translation question. The real8 check covers both sides of the switch
/// exactly (253.149 → ice, 253.15 → liquid). So real4 skips the rows whose
/// input is the threshold's `f32` image (one temperature × three pressures)
/// rather than loosening the bound. The 253.149 rows (`f32` 253.149002) stay
/// in and pass.
#[test]
fn qvsat_matches_flexpart() {
    check_both("qvsat", 1e-14, 0.0, 1e-6, 0.0);
}

/// `richardson.f90` on three synthetic profiles (capped mixed layer, surface
/// stable, deep near-adiabatic) in both the ECMWF-hybrid and NCEP-pressure
/// branches, three heat fluxes (the unstable one runs the excess-temperature
/// iteration) and two `u*`.
///
/// Real4 uses the precision-spread rule for one reason: `hmixplus` divides by
/// the Brunt–Väisälä frequency, built from `theta2 - theta1` across a
/// twentieth of a model layer, which cancels to about four significant digits
/// in `f32`. Three of 36 rows sit `1.1e-4` from the shipped build; `h` and `w*`
/// hold `1e-4` everywhere.
#[test]
fn richardson_matches_flexpart() {
    check_both_rule(
        "richardson",
        1e-12,
        0.0,
        1e-4,
        0.0,
        Real4Rule::PrecisionSpread,
    );
}

// ── solar and geodesy ─────────────────────────────────────────────────────────

/// `zenithangle.f90` over six latitudes, four longitudes and seven dates.
#[test]
fn zenithangle_matches_flexpart() {
    check_both("zenithangle", 1e-13, 0.0, 1e-5, 0.0);
}

/// `photo_O1D.f90` at every integer degree 0–92 plus points next to table
/// nodes and just below 90.
///
/// Real4 uses the precision-spread rule because near the horizon the rate is
/// `exp(-0.4 / cos(sza))`: at 88° the exponent is `-11.5` and the relative
/// error of `cos` in `f32` is amplified by `tan(sza) * sza ≈ 44`, so the
/// shipped build's single-precision error reaches `3e-5` at 87–88°.
#[test]
fn photo_o1d_matches_flexpart() {
    check_both_rule(
        "photo_O1D",
        1e-13,
        0.0,
        1e-5,
        0.0,
        Real4Rule::PrecisionSpread,
    );
}

/// `distance.f90` (degrees), including the `0.03°` zero threshold.
#[test]
fn distance_matches_flexpart() {
    check_both("distance", 1e-14, 0.0, 1e-6, 0.0);
}

/// `distance2.f90` (radians), including the `0.0003 rad` zero threshold.
#[test]
fn distance2_matches_flexpart() {
    check_both("distance2", 1e-14, 0.0, 1e-6, 0.0);
}

// ── calendar ──────────────────────────────────────────────────────────────────

/// `juldate.f90`: the port's Hinnant day count plus FLEXPART's time-of-day
/// arithmetic. Julian dates are `real(kind=dp)` at both precisions upstream, so
/// both fixtures are held to the same bound.
#[test]
fn juldate_matches_flexpart() {
    check_both("juldate", 1e-15, 0.0, 1e-15, 0.0);
}

/// `caldate.f90`, including fractional days on either side of a minute
/// boundary (the `ss == 60` rollover) and the 24:00:00 non-rollover.
#[test]
fn caldate_matches_flexpart() {
    check_both("caldate", 0.0, 0.0, 0.0, 0.0);
}

/// The shipped real(4) `caldate` is wrong at the 1600 century leap day; see
/// [`in_scope`]. Pinned so that a regenerated fixture where it changes is
/// noticed, and so the exclusion above can be removed if it does.
#[test]
fn caldate_real4_century_leap_day_defect_is_upstreams() {
    let rows4 = parse(FIXTURE_REAL4, Precision::Real4);
    let rows8 = parse(FIXTURE_REAL8, Precision::Real8);
    let pick = |rows: &[Row]| -> f64 {
        rows.iter()
            .find(|r| r.function == "caldate" && r.args[0] == 2_305_507.0)
            .expect("1600-02-29 caldate row")
            .outs[0]
    };
    assert_eq!(pick(&rows4), 16_010_231.0, "upstream real4 defect changed");
    assert_eq!(pick(&rows8), 16_000_229.0);
    assert_eq!(caldate(2_305_507.0).0, 16_000_229);
}
