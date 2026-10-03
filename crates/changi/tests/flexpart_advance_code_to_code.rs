// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::advance` against upstream
// FLEXPART v10.4 (commit 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage-4 particle step (`advance`, `initialize`), code-to-code
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_advance.f90` links `advance.f90`, `initialize.f90`,
//! `initialize_cbl_vel.f90`, `re_initialize_particle.f90` and everything they
//! call (`cbl`, `hanna*`, `get_settling`, `windalign`, the ten `interpol_*`
//! routines, `cmapf_mod`) **verbatim**. Two local files stand in for upstream
//! ones, and neither changes a routine under test:
//! - `par_mod.f90` is compiled from a copy with one nest configured;
//! - `dev/random_mod_shim_advance.f90` replaces the Numerical Recipes
//!   generators. It returns the `ran3`/`gasdev` values the driver sets.
//!
//! The Gaussian array `rannumb` is filled by an integer formula that both
//! codes evaluate with a single rounding, so both consume identical draws.
//!
//! Eleven cases sweep the settings:
//! - Gaussian (`hanna`) and well-mixed (`hanna1`) turbulence;
//! - `method` 0 and 1, with 1 and with several vertical sub-steps;
//! - the time-step branches `dt/T_L` below and above 0.5;
//! - the skewed-CBL scheme at strong instability (`-h/L >= 15`), in the sine
//!   taper (`5 < -h/L < 15`) and below it, including velocities far enough in
//!   the tails to force `re_initialize_particle`;
//! - dry deposition and gravitational settling;
//! - backward runs;
//! - a nest and both polar-stereographic grids;
//! - the east-west wrap of a global domain, the pole crossing, and particles
//!   leaving a limited domain (`nstop = 3`).
//!
//! Heights run from 3 m, through the boundary layer and the free troposphere,
//! to the tropopause transition band and the stratosphere.
//!
//! `interpol_mod` and `hanna_mod` carry state from one call to the next
//! upstream, so the test replays every call **in fixture order** with one
//! persistent [`Interpolator`] and [`HannaState`]. Each call starts from the
//! particle state the Fortran recorded for it, so a discrepancy is local to one
//! step and cannot compound. All rows are written in double precision, so the
//! `real(kind=dp)` particle position is exact at both precisions.
//!
//! ## Results
//!
//! Re-print with `cargo test --release -p changi --test
//! flexpart_advance_code_to_code -- --nocapture`; recorded in
//! `docs/flexpart-code-to-code.md`.
//!
//! ## What this does NOT establish
//!
//! Verification of one deterministic step given the draws, on synthetic
//! fields. The distributional behaviour is checked separately, by the
//! stochastic comparison. Nothing here supports emergency response or dose
//! assessment for real populations.

mod common;

use std::collections::HashMap;
use std::sync::OnceLock;

use changi::flexpart::advance::{
    advance, get_vdep_prob, initialize, AdvanceSettings, Domain, NestGeometry, Particle,
    SettlingSpecies,
};
use changi::flexpart::cmapf::Strcmp;
use changi::flexpart::interpolation::{Interpolator, MetFields};
use changi::flexpart::turbulence::HannaState;
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_advance_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_advance_real8.csv");

const NX: usize = 7;
const NY: usize = 7;
const NZ: usize = 10;
const MAXRAND: usize = 1_000_000;
const DP: &[&str] = &[
    "field",
    "height",
    "domain",
    "case",
    "initialize",
    "advance",
    "get_vdep_prob",
];

/// `rannumb(i)` as the driver fills it, at the build's precision.
fn rannumb(p: Precision) -> Vec<f64> {
    (1..=MAXRAND)
        .map(|i| {
            let n = ((i % 2001) * 7919 % 2001) as i64 - 1000;
            match p {
                Precision::Real4 => (n as f32 / 400.0_f32) as f64,
                Precision::Real8 => n as f64 / 400.0,
            }
        })
        .collect()
}

fn build_fields(rows: &[Row]) -> (MetFields, MetFields) {
    let mut mother = MetFields::zeros(NX, NY, NZ, 1);
    mother.height = rows
        .iter()
        .find(|r| r.function == "height")
        .expect("height")
        .args
        .clone();
    let mut nest = mother.clone();
    for r in rows.iter().filter(|r| r.function == "field") {
        let (grid, id, slot, k) = (
            r.args[0] as usize,
            r.args[1] as i32,
            r.args[2] as usize - 1,
            r.args[3] as usize - 1,
        );
        let met = if grid == 0 { &mut mother } else { &mut nest };
        for j in 0..NY {
            for i in 0..NX {
                let v = r.outs[i + NX * j];
                let (i3, i2, iv) = (
                    met.idx3(i, j, k, slot),
                    met.idx2(i, j, slot),
                    met.idx_vdep(i, j, 0, slot),
                );
                match id {
                    1 => met.uu[i3] = v,
                    2 => met.vv[i3] = v,
                    3 => met.ww[i3] = v,
                    4 => met.uupol[i3] = v,
                    5 => met.vvpol[i3] = v,
                    6 => met.rho[i3] = v,
                    7 => met.drhodz[i3] = v,
                    8 => met.tt[i3] = v,
                    11 => met.ustar[i2] = v,
                    12 => met.wstar[i2] = v,
                    13 => met.oli[i2] = v,
                    14 => met.hmix[i2] = v,
                    15 => met.tropopause[i2] = v,
                    16 => met.vdep[iv] = v,
                    _ => panic!("unknown field id {id}"),
                }
            }
        }
    }
    (mother, nest)
}

/// Replay every `initialize` / `advance` call in fixture order; outputs by
/// sequence number.
fn replay(rows: &[Row], p: Precision) -> HashMap<u64, Vec<f64>> {
    let rn = rannumb(p);
    let (mother0, nest0) = build_fields(rows);
    let d = &rows
        .iter()
        .find(|r| r.function == "domain")
        .expect("domain")
        .args;
    let mut north = [0.0; 9];
    let mut south = [0.0; 9];
    north.copy_from_slice(&d[2..11]);
    south.copy_from_slice(&d[11..20]);
    let nest_geo = NestGeometry {
        xln: d[20],
        yln: d[21],
        xrn: d[22],
        yrn: d[23],
        xresoln: d[24],
        yresoln: d[25],
    };
    let mut dom = Domain {
        nxmin1: d[0] as i64,
        nymin1: d[1] as i64,
        nxmax: 361,
        nymax: 181,
        xglobal: false,
        nglobal: false,
        sglobal: false,
        switchnorthg: 0.0,
        switchsouthg: 0.0,
        dx: 0.0,
        dy: 0.0,
        xlon0: 0.0,
        ylat0: 0.0,
        dxconst: 0.0,
        dyconst: 0.0,
        northpolemap: Strcmp(north),
        southpolemap: Strcmp(south),
        nests: Vec::new(),
    };
    let mut cfg: Option<AdvanceSettings> = None;
    let mut species = Vec::new();
    let (mut mother, mut nest) = (mother0.clone(), nest0.clone());
    let mut ip = Interpolator::new(NZ, 1);
    let mut hs = HannaState::default();
    let xmass = [1.0];
    let mut out = HashMap::new();

    for r in rows {
        match r.function.as_str() {
            "case" => {
                let c = &r.args;
                cfg = Some(AdvanceSettings {
                    ldirect: c[1] as i64,
                    lsynctime: c[2] as i64,
                    method: c[3] as i32,
                    ctl: c[4],
                    ifine: c[5] as i64,
                    fine: c[6],
                    mintime: c[7] as i64,
                    turbswitch: c[8] == 1.0,
                    cblflag: c[9] == 1.0,
                    turboff: false,
                    drydep: c[10] == 1.0,
                    drydepspec: vec![c[10] == 1.0],
                    d_trop: c[11],
                    d_strat: c[12],
                    lwindinterv: c[13] as i64,
                    turbmesoscale: c[14],
                    mdomainfill: c[15] as i32,
                    lsettling: c[16] == 1.0,
                    interpolhmix: false,
                    nmixz: c[17] as usize,
                });
                species = vec![SettlingSpecies {
                    density: c[18],
                    dquer: c[19],
                    cunningham: c[20],
                    vsetaver: c[21],
                }];
                dom.xglobal = c[22] == 1.0;
                dom.nglobal = c[23] == 1.0;
                dom.sglobal = c[24] == 1.0;
                dom.switchnorthg = c[25];
                dom.switchsouthg = c[26];
                dom.dx = c[27];
                dom.dy = c[28];
                dom.xlon0 = c[29];
                dom.ylat0 = c[30];
                dom.nests = if c[32] >= 1.0 {
                    vec![nest_geo]
                } else {
                    Vec::new()
                };
                dom.dxconst = r.outs[0];
                dom.dyconst = r.outs[1];
                let (olic, wstc) = (r.outs[2], r.outs[3]);
                mother = mother0.clone();
                nest = nest0.clone();
                for slot in 0..2 {
                    for j in 0..NY {
                        for i in 0..NX {
                            let k = mother.idx2(i, j, slot);
                            if olic != 0.0 {
                                mother.oli[k] = olic;
                            }
                            if wstc != 0.0 {
                                mother.wstar[k] = wstc;
                            }
                        }
                    }
                }
                for m in [&mut mother, &mut nest] {
                    m.memtime = [0, c[31] as i64];
                    m.memind = [0, 1];
                }
            }
            "initialize" => {
                let a = &r.args;
                let cfg = cfg.as_ref().expect("case before calls");
                let mut part = Particle {
                    xt: a[3],
                    yt: a[4],
                    zt: a[5],
                    up: 0.0,
                    vp: 0.0,
                    wp: 0.0,
                    usigold: 0.0,
                    vsigold: 0.0,
                    wsigold: 0.0,
                    icbt: 0,
                    prob: vec![0.0],
                };
                let ldt = initialize(
                    a[2] as i64,
                    &mut part,
                    a[6] as usize,
                    &rn,
                    (a[7], a[8]),
                    &mut ip,
                    &mut hs,
                    &mother,
                    cfg,
                )
                .expect("initialize");
                out.insert(
                    a[0] as u64,
                    vec![
                        part.up,
                        part.vp,
                        part.wp,
                        part.usigold,
                        part.vsigold,
                        part.wsigold,
                        ldt as f64,
                        part.icbt as f64,
                    ],
                );
            }
            "advance" => {
                let a = &r.args;
                let cfg = cfg.as_ref().expect("case before calls");
                let mut part = Particle {
                    xt: a[4],
                    yt: a[5],
                    zt: a[6],
                    up: a[7],
                    vp: a[8],
                    wp: a[9],
                    usigold: a[10],
                    vsigold: a[11],
                    wsigold: a[12],
                    icbt: a[13] as i64,
                    prob: vec![a[14]],
                };
                let mut ldt = a[3] as i64;
                let o = advance(
                    a[2] as i64,
                    &mut ldt,
                    &mut part,
                    a[15] as usize,
                    &rn,
                    &mut ip,
                    &mut hs,
                    &mother,
                    std::slice::from_ref(&nest),
                    &dom,
                    cfg,
                    &species,
                    &xmass,
                )
                .expect("advance");
                out.insert(
                    a[0] as u64,
                    vec![
                        part.xt,
                        part.yt,
                        part.zt,
                        part.up,
                        part.vp,
                        part.wp,
                        part.usigold,
                        part.vsigold,
                        part.wsigold,
                        part.icbt as f64,
                        part.prob[0],
                        ldt as f64,
                        f64::from(o.nstop),
                        o.nan_count as f64,
                        o.nan_count2 as f64,
                        ip.u,
                        ip.v,
                        ip.w,
                        hs.h,
                        f64::from(ip.ngrid),
                    ],
                );
            }
            "get_vdep_prob" => {
                let a = &r.args;
                let cfg = cfg.as_ref().expect("case before calls");
                // The driver resets prob to 0 before each call.
                let mut prob = vec![0.0];
                get_vdep_prob(
                    a[3],
                    a[4],
                    a[5],
                    &mut prob,
                    &mut ip,
                    &mother,
                    std::slice::from_ref(&nest),
                    &dom,
                    cfg,
                );
                out.insert(
                    a[0] as u64,
                    vec![prob[0], f64::from(ip.ngrid), ip.ix as f64, ip.jy as f64],
                );
            }
            _ => {}
        }
    }
    out
}

struct Ctx {
    fx: Fixtures,
    r4: HashMap<u64, Vec<f64>>,
    r8: HashMap<u64, Vec<f64>>,
    /// Per case: (ylat0, dy, uses a polar grid).
    cases: HashMap<u64, (f64, f64, bool)>,
}

fn ctx() -> &'static Ctx {
    static C: OnceLock<Ctx> = OnceLock::new();
    C.get_or_init(|| {
        let fx = Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, DP);
        let r4 = replay(&fx.real4, Precision::Real4);
        let r8 = replay(&fx.real8, Precision::Real8);
        let cases = fx
            .real8
            .iter()
            .filter(|r| r.function == "case")
            .map(|r| {
                (
                    r.args[0] as u64,
                    (
                        r.args[30],
                        r.args[28],
                        r.args[23] == 1.0 || r.args[24] == 1.0,
                    ),
                )
            })
            .collect();
        Ctx { fx, r4, r8, cases }
    })
}

fn evaluate(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let c = ctx();
    let m = if p == Precision::Real4 { &c.r4 } else { &c.r8 };
    m.get(&(r.args[0] as u64)).cloned()
}

fn all(_: &Row, _: Precision) -> bool {
    true
}

/// Real(4) scope of `advance`: every row except polar-grid steps that start
/// within 1° of a pole.
///
/// The stage-`cmapf` verification established, from `cmapf_mod` alone, that
/// the shipped build cannot carry `cxy2ll`/`cg2cxy` poleward of 89°: 89° is
/// upstream's own polar switch in `cg2cxy`, and there its `f32` longitude is
/// off by up to 180°. A particle step on the polar grid goes through
/// `cll2xy`/`cxy2ll`, so from 89° poleward the shipped build's position and
/// the Petterssen winds inherit that error (here up to `5e-4` in `xt`). The
/// cut comes from the cmapf analysis, not from this comparison. The real(8)
/// check covers these rows bit for bit.
fn advance_scope(r: &Row, p: Precision) -> bool {
    if p == Precision::Real8 {
        return true;
    }
    let (ylat0, dy, polar) = ctx().cases[&(r.args[1] as u64)];
    let lat = ylat0 + r.args[5] * dy;
    !(polar && lat.abs() >= 89.0)
}

/// `get_vdep_prob.f90`, called right after each `advance`, so the stale
/// interpolation weights it uses (see its doc) are the ones `advance` left.
#[test]
fn get_vdep_prob_matches_flexpart() {
    check_group(
        &ctx().fx,
        "get_vdep_prob",
        Bounds::rel(1e-13, 1e-5),
        all,
        evaluate,
    );
}

/// `initialize.f90` (+ `initialize_cbl_vel.f90`).
#[test]
fn initialize_matches_flexpart() {
    check_group(
        &ctx().fx,
        "initialize",
        Bounds::rel(1e-12, 1e-4),
        all,
        evaluate,
    );
}

/// `advance.f90`, every case.
///
/// # Real(4): which columns may be attributed to upstream's precision, and why
///
/// Against real(8) every output of every row is bit-exact. Against the
/// shipped build, five columns are allowed the precision-spread rule:
///
/// - **`xt` (column 0) on the polar grids.** The step goes through
///   `cll2xy` → `cxy2ll`, and near the pole the recovered longitude, hence
///   `xt`, amplifies the `f32` error of the stereographic coordinates. The
///   shipped build's `xt` sits up to `4e-4` from the port 1.6° from the pole.
/// - **`u`, `v`, `w` (15–17)**, which leave `advance` as the Petterssen
///   correction `(u_new - u_old) / 2`, a difference of two nearly equal winds
///   (`1e-6` m/s out of `1`).
/// - **`vsigold` (7)**, one polar row where the mesoscale term is
///   `0.0004` m/s.
///
/// The real(4) inputs of each step follow the real(4) trajectory, so the
/// real(4)-vs-real(8) spread here also contains the divergence of the two
/// builds' trajectories. That makes this the weaker of the two checks, and it
/// is stated as such. The real(8) check is the translation proof.
#[test]
fn advance_matches_flexpart() {
    let b = Bounds {
        tol8: 1e-12,
        floor8: 0.0,
        tol4: 1e-4,
        floor4: 0.0,
        rule: Real4Rule::PrecisionSpreadOn(&[0, 7, 15, 16, 17]),
    };
    check_group(&ctx().fx, "advance", b, advance_scope, evaluate);
}
