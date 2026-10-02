// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::cmapf` and
// `changi::flexpart::coordtrafo` against upstream FLEXPART v10.4 (commit
// 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART map projections (`cmapf_mod.f90`) and `coordtrafo.f90`
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_cmapf.f90` (built by
//! `dev/build_reference_cmapf.sh`) links `cmapf_mod.f90`, `coordtrafo.f90`,
//! `par_mod`, `com_mod` and `point_mod` verbatim. Because `cmapf_mod` keeps
//! thirteen of its nineteen routines `private`, the build also links a copy
//! whose only change is the deleted `private` statement (asserted by the
//! script), and the driver checks that both copies agree on every public
//! routine at every point.
//!
//! The sweep:
//! - `cspanf` on and around its wrap edges (begin, end, ±1e-4, several turns)
//!   for four spans, both orders of the bounds;
//! - `eqvlat` on both of its branches, both hemispheres, and `(±90, ±90)`;
//! - `stlmbr` for eight tangent latitudes (±90, 0, ±0.4, 45, -35, 89.99) by
//!   five reference longitudes (including 180, -180, 200);
//! - nine maps: (1, 2) `northpolemap` and `southpolemap` built exactly as
//!   `gridcheck_ecmwf.f90` builds them (`dy = 0.5`); (3) Lambert,
//!   `eqvlat(30, 60)`, `stcm2p`; (4) southern Lambert, tangent -35, rotated
//!   30° by `stcm1p`; (5) Mercator; (6) near-Mercator, tangent 0.4°; (7, 8)
//!   near-polar, tangent ±89.5, so that `|gamma|` lies strictly between
//!   `cgszll`'s 0.9999 guard and 1; (9) a polar map straight from `stlmbr`,
//!   on which `ccrvxy`'s `temp == 0`, `|gamma| == 1` branch is representable;
//! - on maps 1-6, 14 latitudes straddling every pole guard (±89.8, ±89.9,
//!   ±89.985 exactly, ±89.99, ±90) by 9 longitudes straddling the map's cut
//!   and the dateline (8 x 3 on maps 7-9), for the seven ll routines; for the
//!   six xy routines, the images of those points, a 5 x 5 scan around the
//!   pole image, and the exact zero of `ccrvxy`'s polar vector (searched in
//!   the driver, in each build's own arithmetic);
//! - `coordtrafo` on five scenarios (regional; global with both poles;
//!   x-global only; `nymin1 = 1`; boxes within 1e-5° of the north pole),
//!   boxes on every bound, plus the all-removed `stop` path in a separate
//!   process.
//!
//! Every row is printed at 17 significant digits; real(4) values widen to
//! `f64` exactly, so the port is fed what the Fortran was fed.
//!
//! ## Bounds, and why
//!
//! **Real(8): relative 1e-13, no floor, no exclusion, every group.** It
//! measures the translation; every output of every group is bit-exact.
//!
//! **Real(4): relative 1e-5**, with these documented allowances. Each was
//! diagnosed row by row before it was written down; none was tuned to pass.
//!
//! - *Conditioning scope* for `cgszll`, `cc2gll`, `cg2cll`, `ccrvll`, the
//!   routines driven by `cos(lat)`. A row is out of real(4) scope when
//!   `kappa = eps32 |a| |tan a|` (`a` the latitude in radians) exceeds 1e-5:
//!   upstream forms `radpdg*xlat` in `f32`, and `cos` near the pole amplifies
//!   that rounding by `|a tan a|` (2.7e-5 at 89.8°, so |lat| >= 89.8 leaves
//!   scope; `ccrvll` misses there reach 1.9e11 rad/km, since its curvature
//!   divides by `cos`). This also removes every row ON the `|lat| > 89.985`
//!   guard of `cc2gll`/`cg2cll`/`cgszll`, where the `f32` image of 89.985
//!   (89.98500061) is above the port's `f64` literal but equal to upstream's
//!   `f32` one, so the two take different branches: 223 `cc2gll` and 223
//!   `cg2cll` outputs, rotations off by up to 38. Real(8) holds both sides
//!   of the guard bit-exactly.
//! - *Pole scope* for `cxy2ll`, `cg2cxy`, `ccrvxy`, `cpolxy`: points poleward
//!   of 89° (upstream's own polar switch in `cg2cxy`) are out of real(4)
//!   scope. There the direction to the pole image is resolved from an `f32`
//!   grid position (and an `f32` `xi0`/`eta0`, see the `cmapf` module doc).
//!   Measured on all 749 rows, beyond 4x upstream's own spread: 19 `cxy2ll`
//!   (longitude off by up to 180°), 27 `cg2cxy` (a wind of magnitude ~6
//!   rotated by up to 10.9), 48 `ccrvxy` (curvature up to 5e8 rad/km at the
//!   apex) and 8 `cpolxy` outputs (the near-Mercator pole image, where the
//!   shipped build takes `temp >= 1` and returns `enz = 1`, while the port,
//!   given the same `f32` point in `f64`, has `temp` just below 1 and
//!   `exp(ymerc)` overflows to `enz = NaN`). Inside 89° every miss is within
//!   the spread or under a floor.
//! - *Absolute floors* of `1e-5 x scale` where an output passes through zero
//!   by symmetry (e.g. `xi` on the cut line, `sin(lat)` at the equator):
//!   scale 1 for canonical coordinates (Earth radii), grid units, unit
//!   vectors, wind components and degrees, `1/rearth` for curvature. The
//!   harness's `max_rel_dev` ignores deviations under the floor, so the
//!   table below also gives the maximum absolute deviation.
//! - *Precision-spread rule* (opt-in, counted): `cspanf` (the `f32`
//!   `value - first` near a wrap edge), `cnllxy` and `cll2xy` (`sin` of an
//!   `f32` argument near `pi` on the cut line, `1 - cos(gamma dlong)` on the
//!   near-Mercator map, and the near-polar rows, which these two keep in
//!   scope), `cxy2ll` (one equatorial latitude at |x| ~ 300 grid units),
//!   `cg2cxy` (one point 20 grid units from the near-polar apex).
//! - *Cut-line longitudes* in `cxy2ll` are compared as angles (see that test).
//! - *`coordtrafo` scenario 5* is real(8)-only (see `ct_scope`).
//!
//! ## Results
//!
//! Taken 2026-10-02, upstream `3d7eebf`, gfortran 13.3.0, 10 491 rows (real4)
//! and 10 492 rows (real8; one more surviving box in scenario 5).
//!
//! | Group | rows r8 | vs real8 | rows r4 in scope | vs real4: max rel (harness) / max abs |
//! |---|---:|---|---:|---|
//! | `cspanf` | 64 | bit-exact | 64 | 1.25e-1 / 1.5e-5 deg (2 by spread) |
//! | `eqvlat` | 11 | bit-exact | 11 | 3.4e-6 |
//! | `stlmbr` | 49 | bit-exact | 49 | 1.5e-6 |
//! | `stcm2p` | 5 | bit-exact | 5 | 3.2e-7 |
//! | `stcm1p` | 3 | bit-exact | 3 | 3.6e-6 |
//! | `cnllxy` | 828 | bit-exact | 828 | 1.0 / 8.0e-3 (28 by spread) |
//! | `cll2xy` | 828 | bit-exact | 828 | 1.0 / 7.3 grid units (7 by spread) |
//! | `cgszll` | 828 | bit-exact | 234 | 2.8e-7 |
//! | `cc2gll` | 828 | bit-exact | 234 | under floor / 3.6e-6 |
//! | `cg2cll` | 828 | bit-exact | 234 | under floor / 3.6e-6 |
//! | `ccrvll` | 828 | bit-exact | 234 | under floor / 1.4e-10 rad/km |
//! | `cpolll` | 828 | bit-exact | 828 | under floor / 1.9e-7 |
//! | `cnxyll` | 749 | bit-exact | 749 | 1.3e-6 |
//! | `cxy2ll` | 749 | bit-exact | 369 | 2.6 (a latitude near 0) / 1.8e-4 deg (1 by spread) |
//! | `cgszxy` | 749 | bit-exact | 749 | 1.0e-6 |
//! | `cg2cxy` | 749 | bit-exact | 369 | 1.2e-5 / 1.7e-5 (1 by spread) |
//! | `ccrvxy` | 749 | bit-exact | 369 | under floor / 5.3e-10 rad/km |
//! | `cpolxy` | 749 | bit-exact | 369 | under floor / 2.0e-7 |
//! | `coordtrafo` (+ count, stop) | 17 + 5 + 1 | bit-exact | 14 + 4 + 1 | 4.2e-8 |
//!
//! "bit-exact" means every output, including the `NaN` and `inf` rows (the
//! Mercator poles, `eqvlat(±90, ±90)`, the unguarded `cnxyll` pole).
//! Interpretation: the translation is exact. Against FLEXPART as shipped, the
//! port agrees to the `f32` band wherever `f32` can carry the answer, and the
//! allowances above say where it cannot. Those places are near the poles,
//! which is exactly where FLEXPART uses these maps (poleward of
//! `switchnorth`/`switchsouth`).

//! ## The suite is not vacuous
//!
//! 40 mutations (one or more per routine, both files), 38 killed, each by
//! the group(s) that own the mutated code; the table is in the stage report.
//! The two survivors change the last coefficient of a series: `cnllxy`'s
//! `1/42` (contributes < 2e-16 relative over the sweep, invisible in `f64`)
//! and `cnxyll`'s `1/7` (moves 8 outputs by at most 8.9e-15, under the
//! 1e-13 real(8) bound). The first pass also left two sweep gaps, closed by
//! maps 7-9: `cgszll`'s `|gamma| > 0.9999` placement and `ccrvxy`'s
//! `|gamma| == 1` zero branch.
//!
//! ## What this does NOT establish
//!
//! Verification, not validation. Nothing here supports emergency response or
//! dose assessment for real populations.

mod common;

use std::sync::OnceLock;

use changi::flexpart::cmapf::{self, Strcmp};
use changi::flexpart::coordtrafo::{coordtrafo, ReleaseBox, ReleaseGrid};
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_cmapf_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_cmapf_real8.csv");

/// Every row mixes default real and real(kind=dp) and is printed at 17
/// digits, so every function is parsed as f64.
const ALL: &[&str] = &[
    "cspanf",
    "eqvlat",
    "stlmbr",
    "stcm2p",
    "stcm1p",
    "map",
    "cnllxy",
    "cll2xy",
    "cgszll",
    "cc2gll",
    "cg2cll",
    "ccrvll",
    "cpolll",
    "cnxyll",
    "cxy2ll",
    "cgszxy",
    "cg2cxy",
    "ccrvxy",
    "cpolxy",
    "ctgrid",
    "ctin",
    "coordtrafo",
    "coordtrafo_n",
    "coordtrafo_stop",
];

fn fx() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, ALL))
}

/// The map a row refers to, as that build stored it.
fn map(p: Precision, id: f64) -> Strcmp {
    let r = fx()
        .rows(p)
        .iter()
        .find(|r| r.function == "map" && r.args[0] == id)
        .expect("map row");
    let mut s = [0.0; 9];
    s.copy_from_slice(&r.outs);
    Strcmp(s)
}

fn strcmp_from(a: &[f64]) -> Strcmp {
    let mut s = [0.0; 9];
    s.copy_from_slice(&a[..9]);
    Strcmp(s)
}

/// Real(4) relative bound.
const TOL4: f64 = 1e-5;
const TIGHT: Bounds = Bounds::rel(1e-13, TOL4);

/// `TIGHT`, plus a real(4) absolute floor of `TOL4 x scale` for outputs that
/// pass through zero (see "Absolute floors" in the module documentation).
const fn floored(scale: f64, rule: Real4Rule) -> Bounds {
    Bounds {
        tol8: 1e-13,
        floor8: 0.0,
        tol4: TOL4,
        floor4: TOL4 * scale,
        rule,
    }
}
/// Earth radius as upstream writes it, km.
const REARTH: f64 = 6371.2;

fn all(_: &Row, _: Precision) -> bool {
    true
}

#[test]
fn cspanf() {
    check_group(
        fx(),
        "cspanf",
        floored(1.0, Real4Rule::PrecisionSpread),
        all,
        |r, _| Some(vec![cmapf::cspanf(r.args[0], r.args[1], r.args[2])]),
    );
}

#[test]
fn eqvlat() {
    check_group(
        fx(),
        "eqvlat",
        floored(1.0, Real4Rule::Strict),
        all,
        |r, _| Some(vec![cmapf::eqvlat(r.args[0], r.args[1])]),
    );
}

#[test]
fn stlmbr() {
    check_group(fx(), "stlmbr", TIGHT, all, |r, _| {
        Some(cmapf::stlmbr(r.args[0], r.args[1]).0.to_vec())
    });
}

#[test]
fn stcm2p() {
    check_group(fx(), "stcm2p", TIGHT, all, |r, _| {
        let a = &r.args;
        let mut s = strcmp_from(a);
        cmapf::stcm2p(
            &mut s, a[9], a[10], a[11], a[12], a[13], a[14], a[15], a[16],
        );
        Some(s.0.to_vec())
    });
}

#[test]
fn stcm1p() {
    check_group(fx(), "stcm1p", TIGHT, all, |r, _| {
        let a = &r.args;
        let mut s = strcmp_from(a);
        cmapf::stcm1p(
            &mut s, a[9], a[10], a[11], a[12], a[13], a[14], a[15], a[16],
        );
        Some(s.0.to_vec())
    });
}

/// `f32` unit roundoff.
const EPS32: f64 = f32::EPSILON as f64 / 2.0;

/// Relative conditioning of `cos(radpdg*lat)` when the argument is formed in
/// `f32`: `eps32 * |a| * |tan a|`, `a` in radians.
fn kappa_cos(lat: f64) -> f64 {
    let a = lat.to_radians();
    EPS32 * a.abs() * a.tan().abs()
}

/// Real(4) scope of the routines taking a latitude: rows where upstream's
/// single precision cannot hold `tol4` by conditioning alone are left to the
/// real(8) check. See the module documentation.
fn ll_scope(r: &Row, p: Precision) -> bool {
    p == Precision::Real8 || kappa_cos(r.args[1]) <= TOL4
}

/// Real(4) scope of the routines taking a map position: points poleward of
/// 89 degrees (upstream's own polar switch in `cg2cxy`) are left to the
/// real(8) check. NaN latitudes (Mercator pole images) stay in scope.
fn xy_scope(r: &Row, p: Precision) -> bool {
    let lat = cmapf::cxy2ll(&map(p, r.args[0]), r.args[1], r.args[2]).0;
    p == Precision::Real8 || lat.is_nan() || lat.abs() <= 89.0
}

/// Rows `map; a1; a2[; u; v]`.
fn map_group<S: Fn(&Row, Precision) -> bool>(
    name: &str,
    b: Bounds,
    scope: S,
    f: fn(&Strcmp, &[f64]) -> Vec<f64>,
) {
    check_group(fx(), name, b, scope, |r, p| {
        let s = map(p, r.args[0]);
        Some(f(&s, &r.args[1..]))
    });
}

#[test]
fn cnllxy() {
    map_group(
        "cnllxy",
        floored(1.0, Real4Rule::PrecisionSpread),
        all,
        |s, a| {
            let (x, y) = cmapf::cnllxy(s, a[0], a[1]);
            vec![x, y]
        },
    );
}

#[test]
fn cll2xy() {
    map_group(
        "cll2xy",
        floored(1.0, Real4Rule::PrecisionSpread),
        all,
        |s, a| {
            let (x, y) = cmapf::cll2xy(s, a[0], a[1]);
            vec![x, y]
        },
    );
}

#[test]
fn cgszll() {
    map_group("cgszll", TIGHT, ll_scope, |s, a| {
        vec![cmapf::cgszll(s, a[0], a[1])]
    });
}

#[test]
fn cc2gll() {
    map_group(
        "cc2gll",
        floored(1.0, Real4Rule::Strict),
        ll_scope,
        |s, a| {
            let (u, v) = cmapf::cc2gll(s, a[0], a[1], a[2], a[3]);
            vec![u, v]
        },
    );
}

#[test]
fn cg2cll() {
    map_group(
        "cg2cll",
        floored(1.0, Real4Rule::Strict),
        ll_scope,
        |s, a| {
            let (u, v) = cmapf::cg2cll(s, a[0], a[1], a[2], a[3]);
            vec![u, v]
        },
    );
}

#[test]
fn ccrvll() {
    map_group(
        "ccrvll",
        floored(1.0 / REARTH, Real4Rule::Strict),
        ll_scope,
        |s, a| {
            let (x, y) = cmapf::ccrvll(s, a[0], a[1]);
            vec![x, y]
        },
    );
}

#[test]
fn cpolll() {
    map_group("cpolll", floored(1.0, Real4Rule::Strict), all, |s, a| {
        let (x, y, z) = cmapf::cpolll(s, a[0], a[1]);
        vec![x, y, z]
    });
}

#[test]
fn cnxyll() {
    map_group("cnxyll", TIGHT, all, |s, a| {
        let (x, y) = cmapf::cnxyll(s, a[0], a[1]);
        vec![x, y]
    });
}

/// On the map's cut line the longitude is 180 up to the last bit, and
/// `cspanf`'s `(-180, 180]` wrap then decides between `180` and `-180 + 1e-13`.
/// Against the real(4) build (whose `xlong` is rounded to `f32` before the
/// wrap) the port's longitude is therefore compared as an angle: shifted by
/// 360 when it lies on the other side of the cut from FLEXPART's. The real(8)
/// comparison takes no such allowance and is bit-exact.
#[test]
fn cxy2ll() {
    check_group(
        fx(),
        "cxy2ll",
        floored(1.0, Real4Rule::PrecisionSpread),
        xy_scope,
        |r, p| {
            let s = map(p, r.args[0]);
            let (lat, mut lon) = cmapf::cxy2ll(&s, r.args[1], r.args[2]);
            if p == Precision::Real4 && (lon - r.outs[1]).abs() > 180.0 {
                lon -= 360.0_f64.copysign(lon - r.outs[1]);
            }
            Some(vec![lat, lon])
        },
    );
}

#[test]
fn cgszxy() {
    map_group("cgszxy", TIGHT, all, |s, a| {
        vec![cmapf::cgszxy(s, a[0], a[1])]
    });
}

#[test]
fn cg2cxy() {
    map_group(
        "cg2cxy",
        floored(1.0, Real4Rule::PrecisionSpread),
        xy_scope,
        |s, a| {
            let (u, v) = cmapf::cg2cxy(s, a[0], a[1], a[2], a[3]);
            vec![u, v]
        },
    );
}

#[test]
fn ccrvxy() {
    map_group(
        "ccrvxy",
        floored(1.0 / REARTH, Real4Rule::Strict),
        xy_scope,
        |s, a| {
            let (x, y) = cmapf::ccrvxy(s, a[0], a[1]);
            vec![x, y]
        },
    );
}

#[test]
fn cpolxy() {
    map_group(
        "cpolxy",
        floored(1.0, Real4Rule::Strict),
        xy_scope,
        |s, a| {
            let (x, y, z) = cmapf::cpolxy(s, a[0], a[1]);
            vec![x, y, z]
        },
    );
}

// ---------------------------------------------------------------------------
// coordtrafo

/// Run the port on scenario `scen` as the build `p` set it up.
fn scenario(
    p: Precision,
    scen: f64,
) -> Result<Vec<(usize, ReleaseBox)>, changi::flexpart::coordtrafo::NoReleasePoints> {
    let rows = fx().rows(p);
    let g = &rows
        .iter()
        .find(|r| r.function == "ctgrid" && r.args[0] == scen)
        .expect("ctgrid row")
        .args;
    let grid = ReleaseGrid {
        xlon0: g[1],
        ylat0: g[2],
        dx: g[3],
        dy: g[4],
        nxmin1: g[5] as i32,
        nymin1: g[6] as i32,
        xglobal: g[7] == 1.0,
        sglobal: g[8] == 1.0,
        nglobal: g[9] == 1.0,
    };
    let pts: Vec<ReleaseBox> = rows
        .iter()
        .filter(|r| r.function == "ctin" && r.args[0] == scen)
        .map(|r| ReleaseBox {
            xpoint1: r.args[2],
            ypoint1: r.args[3],
            xpoint2: r.args[4],
            ypoint2: r.args[5],
        })
        .collect();
    coordtrafo(&grid, &pts)
}

/// Scenario 5 places box edges within 1e-5 deg of the north pole, on the
/// 1e-6 / 1e-5 / `spacing` margins of the domain test, where the shipped
/// build's `f32` rounding of `(lat - ylat0)/dy` (spacing 1.5e-5 at 180)
/// decides the outcome. It is checked against real(8) only.
fn ct_scope(r: &Row, p: Precision) -> bool {
    p == Precision::Real8 || r.args[0] != 5.0
}

#[test]
fn coordtrafo_survivors() {
    check_group(fx(), "coordtrafo", TIGHT, ct_scope, |r, p| {
        let kept = scenario(p, r.args[0]).expect("survivors");
        let k = r.args[1] as usize - 1;
        Some(match kept.get(k) {
            Some((i, b)) => vec![(*i + 1) as f64, b.xpoint1, b.ypoint1, b.xpoint2, b.ypoint2],
            None => vec![f64::INFINITY; 5],
        })
    });
}

#[test]
fn coordtrafo_count() {
    check_group(fx(), "coordtrafo_n", TIGHT, ct_scope, |r, p| {
        Some(vec![scenario(p, r.args[0]).map_or(0, |v| v.len()) as f64])
    });
}

/// Upstream executes `stop` when every box is removed; the build script
/// records that as `coordtrafo_stop,<scen>,1`.
#[test]
fn coordtrafo_stop() {
    check_group(fx(), "coordtrafo_stop", TIGHT, all, |r, p| {
        Some(vec![if scenario(p, r.args[0]).is_err() {
            1.0
        } else {
            0.0
        }])
    });
}
