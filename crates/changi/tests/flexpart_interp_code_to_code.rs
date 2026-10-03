// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::interpolation` against
// upstream FLEXPART v10.4 (commit 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage-2 meteorological interpolation, code-to-code
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_interp.f90` links upstream's ten interpolation
//! routines verbatim: `interpol_all`, `interpol_misslev`, `interpol_wind`,
//! `interpol_wind_short` and `interpol_vdep`, each with its `_nests`
//! counterpart. It writes synthetic wind, density, surface-scale and
//! deposition fields into `com_mod`, on a 7 x 7 patch and 8 levels at both
//! time levels, for the mother grid and one nest, and echoes every field as a
//! `field` setup row. The nest needs one configuration change: the build
//! compiles a copy of `par_mod.f90` with `maxnests=1, nxmaxn=12, nymaxn=12`
//! instead of the shipped zeros. That file is FLEXPART's user-edited
//! configuration; every routine under test is verbatim.
//!
//! The sweep covers:
//! - six positions, including cell corners (`ddx = 0`) and the column where
//!   the interpolated `1/L` is exactly 0 (`ol = 99999`);
//! - six heights, including exactly on a level and just below the top;
//! - two times, including `itime = memtime(1)`;
//! - both orders of `memind`;
//! - the mother grid, the north-polar grid (`ngrid = -1`, which reads
//!   `uupol`/`vvpol`) and nest 1.
//!
//! `ww` is constant across part of the patch, so the zero-variance guard of
//! the sub-grid standard deviation is exercised.
//!
//! Each port function is checked against the mother-grid routine AND its
//! `_nests` twin: a `_nests` row is evaluated by the same Rust function on the
//! nest's fields.
//!
//! ## Results
//!
//! Re-print with `cargo test --release -p changi --test
//! flexpart_interp_code_to_code -- --nocapture`; recorded in
//! `docs/flexpart-code-to-code.md`.
//!
//! ## What this does NOT establish
//!
//! Verification, not validation, on synthetic fields. Nothing here supports
//! emergency response or dose assessment for real populations.

mod common;

use std::sync::OnceLock;

use changi::flexpart::interpolation::{Interpolator, MetFields};
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_interp_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_interp_real8.csv");

const NX: usize = 7;
const NY: usize = 7;
const NZ: usize = 8;

struct Grids {
    mother: MetFields,
    nest: MetFields,
}

fn build_grids(rows: &[Row]) -> Grids {
    let mut mother = MetFields::zeros(NX, NY, NZ, 1);
    let height = rows
        .iter()
        .find(|r| r.function == "height")
        .expect("height row")
        .args
        .clone();
    mother.height = height;
    mother.memtime = [0, 10800];
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
    Grids { mother, nest }
}

struct Setup {
    fx: Fixtures,
    grids4: Grids,
    grids8: Grids,
}

fn setup() -> &'static Setup {
    static S: OnceLock<Setup> = OnceLock::new();
    S.get_or_init(|| {
        let fx = Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, &[]);
        let grids4 = build_grids(&fx.real4);
        let grids8 = build_grids(&fx.real8);
        Setup { fx, grids4, grids8 }
    })
}

/// Fields and an interpolator positioned as `advance.f90` positions it.
/// Row args: grid, memind(1), itime, xt, yt, zt[, level].
fn positioned(r: &Row, p: Precision) -> (MetFields, Interpolator) {
    let s = setup();
    let grids = if p == Precision::Real4 {
        &s.grids4
    } else {
        &s.grids8
    };
    let g = r.args[0] as i32;
    let mut met = if g > 0 {
        grids.nest.clone()
    } else {
        grids.mother.clone()
    };
    met.memind = if r.args[1] == 1.0 { [0, 1] } else { [1, 0] };
    let mut ip = Interpolator::new(NZ, 1);
    let (xt, yt) = (r.args[3], r.args[4]);
    ip.ix = xt as usize;
    ip.jy = yt as usize;
    ip.ixp = ip.ix + 1;
    ip.jyp = ip.jy + 1;
    ip.ngrid = g;
    (met, ip)
}

fn profile(ip: &Interpolator, n: usize) -> [f64; 8] {
    [
        ip.uprof[n],
        ip.vprof[n],
        ip.wprof[n],
        ip.rhoprof[n],
        ip.rhogradprof[n],
        ip.usigprof[n],
        ip.vsigprof[n],
        ip.wsigprof[n],
    ]
}

fn evaluate(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let (met, mut ip) = positioned(r, p);
    let (itime, xt, yt, zt) = (r.args[2] as i64, r.args[3], r.args[4], r.args[5]);
    Some(match r.function.as_str() {
        "interpol_all" => {
            let s = ip
                .interpol_all(&met, itime, xt, yt, zt)
                .expect("zt below top");
            let mut o = vec![
                s.ust,
                s.wst,
                s.ol,
                (ip.indz + 1) as f64,
                (ip.indzp + 1) as f64,
            ];
            o.extend(profile(&ip, ip.indz));
            o.extend(profile(&ip, ip.indzp));
            // 1 = flag still set; the driver raises them all before the call
            o.push(f64::from(u8::from(ip.indzindicator[ip.indz])));
            o.push(f64::from(u8::from(ip.indzindicator[ip.indzp])));
            o
        }
        "interpol_misslev" => {
            // The driver calls interpol_all first (setting the weights), then
            // interpol_misslev for each level.
            ip.interpol_all(&met, itime, xt, yt, zt)
                .expect("zt below top");
            let n = r.args[6] as usize - 1;
            ip.indzindicator[n] = true;
            ip.interpol_misslev(&met, n);
            let mut o = profile(&ip, n).to_vec();
            o.push(f64::from(u8::from(ip.indzindicator[n])));
            o
        }
        "interpol_vdep" => {
            ip.interpol_all(&met, itime, xt, yt, zt)
                .expect("zt below top");
            ip.depoindicator[0] = true;
            let vd = ip.interpol_vdep(&met, 0);
            vec![vd, f64::from(u8::from(ip.depoindicator[0]))]
        }
        "interpol_wind" => {
            ip.usig = -1.0;
            ip.vsig = -1.0;
            ip.wsig = -1.0;
            ip.interpol_wind(&met, itime, xt, yt, zt)
                .expect("zt below top");
            vec![
                ip.u,
                ip.v,
                ip.w,
                ip.usig,
                ip.vsig,
                ip.wsig,
                (ip.indz + 1) as f64,
            ]
        }
        "interpol_wind_short" => {
            // Sentinel: interpol_wind_short must leave the sigmas untouched.
            ip.usig = p.lit(-2.0);
            ip.vsig = p.lit(-2.0);
            ip.wsig = p.lit(-2.0);
            ip.interpol_wind_short(&met, itime, xt, yt, zt)
                .expect("zt below top");
            vec![
                ip.u,
                ip.v,
                ip.w,
                ip.usig,
                ip.vsig,
                ip.wsig,
                (ip.indz + 1) as f64,
            ]
        }
        _ => return None,
    })
}

fn all(_: &Row, _: Precision) -> bool {
    true
}

/// Bounds for the groups that output the sub-grid standard deviations; the
/// spread rule applies to the sigma columns `cols` only.
///
/// # Why real(4) needs the precision-spread rule for them
///
/// Upstream computes the corner-value standard deviation with the one-pass
/// formula `sqrt((sum x^2 - (sum x)^2 / n) / (n - 1))`. Its relative error is
/// about `eps * mean^2 / variance`, so it degrades badly in `f32` wherever the
/// spread is small against the mean. In this sweep the shipped build's sigmas
/// miss the real(8) build's by up to `5e-5` relative where the field varies
/// smoothly. Where `ww` is uniform (`0.01` m/s across the cell), the shipped
/// build reports `wsig = 5.0e-6` m/s instead of `0`: the `f32` cancellation
/// leaves `xaux ~ 1.7e-10`, above the `1e-30` guard. The real(8) build and the
/// port agree bit for bit on every one of these outputs, the uniform-field
/// zeros included.
const fn sigma_bounds(cols: &'static [usize]) -> Bounds {
    Bounds {
        tol8: 1e-13,
        floor8: 0.0,
        tol4: 1e-5,
        floor4: 0.0,
        rule: Real4Rule::PrecisionSpreadOn(cols),
    }
}

/// `interpol_all` (+ `_nests`): surface scales and both bracketing levels.
#[test]
fn interpol_all_matches_flexpart() {
    check_group(
        &setup().fx,
        "interpol_all",
        sigma_bounds(&[10, 11, 12, 18, 19, 20]),
        all,
        evaluate,
    );
}

/// `interpol_misslev` (+ `_nests`) at every level.
#[test]
fn interpol_misslev_matches_flexpart() {
    check_group(
        &setup().fx,
        "interpol_misslev",
        sigma_bounds(&[5, 6, 7]),
        all,
        evaluate,
    );
}

/// `interpol_vdep` (+ `_nests`).
#[test]
fn interpol_vdep_matches_flexpart() {
    check_group(
        &setup().fx,
        "interpol_vdep",
        Bounds::rel(1e-13, 1e-5),
        all,
        evaluate,
    );
}

/// `interpol_wind` (+ `_nests`), including the 16-value sub-grid sigma.
#[test]
fn interpol_wind_matches_flexpart() {
    check_group(
        &setup().fx,
        "interpol_wind",
        sigma_bounds(&[3, 4, 5]),
        all,
        evaluate,
    );
}

/// `interpol_wind_short` (+ `_nests`), which must not touch the sigmas.
#[test]
fn interpol_wind_short_matches_flexpart() {
    check_group(
        &setup().fx,
        "interpol_wind_short",
        Bounds::rel(1e-13, 1e-5),
        all,
        evaluate,
    );
}
