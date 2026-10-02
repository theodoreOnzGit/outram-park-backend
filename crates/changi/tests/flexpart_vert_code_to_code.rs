// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::{verttransform,
// shift_field}` against upstream FLEXPART v10.4 (commit 3d7eebf,
// GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage `vert` — the vertical transformation and the field shift
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_vert.f90` links upstream's `verttransform_ecmwf`,
//! `verttransform_gfs`, `verttransform_nests`, `shift_field` and
//! `shift_field_0` **verbatim**, with `cmapf_mod`, `ew` and `qvsat`. It writes
//! synthetic raw level fields into `com_mod` (and the routines' `uuh`, `vvh`,
//! `pvh`, `wwh` arguments), built in double precision and rounded once to
//! `real(4)` in both builds, and echoes every input as a setup row
//! (`vt.grid`, `vt.in`, `vt.nset`). The only local change to upstream is a
//! `par_mod.f90` copy with `maxnests=2, nxmaxn=12, nymaxn=12`.
//!
//! The three routines keep state between calls (the `SAVE`d `init` that sets
//! `height`/`nmixz` once, GFS's static `uvwzlev`) and leave parts of the
//! `com_mod` outputs stale (`uupol` outside the polar bands, `clouds` level
//! 1, `cloudsh` under the cloud-water scheme, GFS `ww` levels the bracket
//! search misses). The driver therefore runs **process sets**: each is one
//! process, starts from sentinel-filled outputs (reals -7, `clouds` 7,
//! `cloudsh` 3, `nmixz` 99), and makes a fixed sequence of calls (`vt.call`
//! rows). This test replays each sequence in order through the port, with
//! the same carried state, and compares every output column after every
//! call:
//!
//! * **p1**, a global 5 x 5 grid with both poles: ECMWF with the RH cloud
//!   scheme (first call: `init`), ECMWF cloud water with separate ice, ECMWF
//!   cloud water summed (accumulating `cloudsh` on the slot of the first
//!   call), GFS (its own `init`, which overwrites `height`) RH scheme, GFS
//!   cloud water, GFS again on the other slot (stale `uvwzlev`, stale `ww`);
//!   then `verttransform_nests` twice on two nests with every
//!   `readclouds_nest`/`sumclouds_nest` combination. Mountain columns
//!   (`ps` 48 000 Pa, 70 000 Pa) make reference heights exceed the column top
//!   (the copy branch, extrapolation with a stale bracket, the GFS stale
//!   `kl`) and give GFS `llev > 1`.
//! * **p2**, 5 x 4 grids: a limited-area ECMWF grid whose heights stay below
//!   `hmixmax` (`nmixz` keeps its 99), with `nwz = nuvz - 1`; a grid with
//!   only the South Pole (its `northpolemap` is all zeros, quirk 6) and one
//!   with only the North Pole, placed so both `ddpol` wraps (`< 0`,
//!   `> 2 pi`) occur; the same three for GFS, including a column whose
//!   `llev` is capped at `nuvz - 2`.
//! * **shift**: `shift_field_0` and `shift_field` for `nxshift` 0, 1, 3, 5,
//!   6 on a 6-column field.
//!
//! The pole reference columns carry `vv = 0` at level 1 and a sign that
//! varies by data set above it, so every `ddpol` branch is taken by both
//! routines and both poles. The undefined-behaviour inputs the port refuses
//! (`VertError`) are avoided by construction; the replay would fail loudly
//! if the fixture hit one.
//!
//! Built twice by `dev/build_reference_vert.sh`: real(4) as FLEXPART ships,
//! and `-fdefault-real-8 -fdefault-double-8`. The real(8) comparison proves
//! the translation; the real(4) comparison measures FLEXPART as shipped.
//!
//! ## Results
//!
//! Taken **2026-10-02**, upstream `3d7eebf`, 14 calls in two process sets
//! plus 10 shift calls; re-print with `cargo test --release -p changi --test
//! flexpart_vert_code_to_code -- --nocapture`.
//!
//! | Group | Rows | Outputs | vs real(8) | vs real(4) |
//! |---|---:|---:|---|---|
//! | `ecmwf.out` | 135 | 12 960 | **bit-exact** | 2.7e-3 max; 406 outputs by the spread rule (`vv`, `ww`, `qv`, `pv`, `drhodz`) |
//! | `ecmwf.cloud` | 135 | 4 590 | **bit-exact** | 8.2e-3 max; 70 by the spread rule (`clwc`, `ciwc`, `clw`); cloud codes and `cloudsh` exact |
//! | `ecmwf.zgrid` | 6 | 54 | **bit-exact** | 2.4e-6 |
//! | `gfs.out` | 135 | 12 960 | **bit-exact** | 275 by the spread rule; 5.9e-3 max away from the pole `ww` that cancels to ~0 |
//! | `gfs.cloud` | 135 | 4 590 | **bit-exact** | 8.2e-3 max; 74 by the spread rule; cloud codes and `cloudsh` exact |
//! | `gfs.zgrid` | 6 | 54 | **bit-exact** | 1.8e-6 |
//! | `nests.out` | 64 | 4 096 | **bit-exact** | 4.5e-4 max; 70 by the spread rule |
//! | `nests.cloud` | 64 | 2 176 | **bit-exact** | 1.6e-5 max; 10 by the spread rule |
//! | `shift_field_0` | 5 | 140 | **bit-exact** | **bit-exact** |
//! | `shift_field` | 5 | 840 | **bit-exact** | **bit-exact** |
//!
//! **Interpretation.** The translation is exact: every one of the 42 460
//! outputs agrees bit for bit with the real(8) build, including every stale
//! value the routines leave behind, the GFS stale bracket and stale
//! `uvwzlev`, and the cloud-base carry-over between columns and nests.
//! Against the shipped real(4) build, the heights hold to 2.4e-6, and every
//! larger miss lies in a continuous field whose `f32` evaluation is
//! ill-conditioned (see `OUT_SPREAD`, `CLOUD_SPREAD`). All of them fall within
//! 4x upstream's own real(4)-vs-real(8) distance, so they are FLEXPART's
//! single precision. No integer output (cloud type, truncated cloud height,
//! `nmixz`) differs between the builds.
//!
//! **Non-vacuity**, mutation run 2026-10-02: 41 mutations of the port. 39
//! were killed, and two survived: removing the south `ddpol < 0` and
//! `ddpol > 2 pi` wraps. Both wraps are reached (a `panic!` probe on each
//! branch, for each routine and each pole, fails the suite). Removing them
//! is equivalent up to rounding, since `ddpol` only enters `sin`/`cos`. The
//! first pass also left the nest-to-nest `cloudh_min` carry-over and the
//! north `ddpol < 0` wrap unreached. The driver was widened (data set 14
//! starts with a cloud-free precipitating column; the global grid starts at
//! `xlon0 = 0`; data set 9 changes the sign of `v` by level) and both are
//! now killed or reached.
//!
//! ## What this does NOT establish
//!
//! Verification, not validation, on synthetic fields. The GRIB readers that
//! fill the raw fields (`readwind_*`) are not ported. Nothing here supports
//! emergency response or dose assessment for real populations.

mod common;

use std::collections::HashMap;
use std::sync::OnceLock;

use changi::flexpart::cmapf::Strcmp;
use changi::flexpart::met_fields::{Field2, Field3, GridGeometry};
use changi::flexpart::shift_field::{shift_field, shift_field_0};
use changi::flexpart::verttransform::{
    verttransform_ecmwf, verttransform_gfs, verttransform_nests, CloudScheme, GfsSaved,
    HybridCoefficients, NestInput, PolarCaps, RawFields, SavedInit, VertGrid, ZFields, ZGrid,
};
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_vert_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_vert_real8.csv");

/// Levels printed per field (`KZ` in the driver).
const KZ: usize = 8;
/// Capacity of the replayed `com_mod` arrays (the driver prefills 0:7).
const CAP: usize = 8;
/// Capacity of the nest arrays (`nxmaxn = nymaxn = 12`).
const NCAP: usize = 12;

fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, &[]))
}

// ── the replay ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct GridDef {
    grid: VertGrid,
    polar: PolarCaps,
    hyb: HybridCoefficients,
}

#[derive(Clone)]
struct NestDef {
    l: usize,
    geom: GridGeometry,
    xresoln: f64,
    yresoln: f64,
}

/// One column of a data set: ps, tt2, td2, lsp, convp, then 8 levels each of
/// uuh, vvh, pvh, tth, qvh, clwch, ciwch, wwh.
type Column = Vec<f64>;

/// Snapshot of what one call left behind.
#[allow(clippy::large_enum_variant)] // no `Box` (workspace rule); a handful of snapshots
enum Snapshot {
    Mother { slot: ZFields, zgrid: ZGrid },
    Nests { nests: Vec<ZFields> },
}

struct Replay {
    snaps: HashMap<(usize, usize), Snapshot>,
}

fn u(x: f64) -> usize {
    x as usize
}

fn parse_grid(a: &[f64]) -> GridDef {
    let o = a;
    let nuvz = u(o[2]);
    let nz = u(o[4]);
    let mut npm = [0.0; 9];
    let mut spm = [0.0; 9];
    npm.copy_from_slice(&o[15..24]);
    spm.copy_from_slice(&o[24..33]);
    let mut i = 33;
    let mut take = |n: usize| {
        let v = o[i..i + n].to_vec();
        i += n;
        v
    };
    let akz = take(nuvz);
    let bkz = take(nuvz);
    let aknew = take(nz);
    let bknew = take(nz);
    GridDef {
        grid: VertGrid {
            geom: GridGeometry {
                nx: u(o[0]),
                ny: u(o[1]),
                dx: o[5],
                dy: o[6],
                xlon0: o[7],
                ylat0: o[8],
            },
            dxconst: o[9],
            dyconst: o[10],
            nuvz,
            nwz: u(o[3]),
            nz,
        },
        polar: PolarCaps {
            nglobal: o[11] != 0.0,
            sglobal: o[12] != 0.0,
            switchnorthg: o[13],
            switchsouthg: o[14],
            northpolemap: Strcmp(npm),
            southpolemap: Strcmp(spm),
        },
        hyb: HybridCoefficients {
            akz,
            bkz,
            aknew,
            bknew,
        },
    }
}

fn raw_fields(set: &HashMap<(usize, usize), Column>, nx: usize, ny: usize) -> RawFields {
    let f3 = |field: usize| {
        let mut f = Field3::filled(nx, ny, KZ, 0.0);
        for jy in 0..ny {
            for ix in 0..nx {
                let c = &set[&(ix, jy)];
                for k in 0..KZ {
                    f.set(ix, jy, k, c[5 + field * KZ + k]);
                }
            }
        }
        f
    };
    let f2 = |i: usize| {
        let mut f = Field2::filled(nx, ny, 0.0);
        for jy in 0..ny {
            for ix in 0..nx {
                f.set(ix, jy, set[&(ix, jy)][i]);
            }
        }
        f
    };
    RawFields {
        uuh: f3(0),
        vvh: f3(1),
        pvh: f3(2),
        tth: f3(3),
        qvh: f3(4),
        clwch: f3(5),
        ciwch: f3(6),
        wwh: f3(7),
        ps: f2(0),
        tt2: f2(1),
        td2: f2(2),
        lsprec: f2(3),
        convprec: f2(4),
    }
}

fn sentinel(nx: usize, ny: usize) -> ZFields {
    ZFields::filled(nx, ny, KZ, -7.0, 7, 3)
}

fn replay(rows: &[Row]) -> Replay {
    let mut grids: HashMap<usize, GridDef> = HashMap::new();
    let mut sets: HashMap<usize, HashMap<(usize, usize), Column>> = HashMap::new();
    let mut nsets: HashMap<usize, NestDef> = HashMap::new();
    let mut snaps = HashMap::new();

    let mut slots = [sentinel(CAP, CAP), sentinel(CAP, CAP)];
    let mut nest_slots = vec![
        [sentinel(NCAP, NCAP), sentinel(NCAP, NCAP)],
        [sentinel(NCAP, NCAP), sentinel(NCAP, NCAP)],
    ];
    let mut ecmwf_saved = SavedInit::default();
    let mut gfs_saved = GfsSaved::new(CAP, CAP, KZ);
    let mut zgrid = ZGrid {
        height: vec![0.0; KZ],
        nmixz: 99,
    };

    for r in rows {
        match r.function.as_str() {
            "vt.proc" => {
                slots = [sentinel(CAP, CAP), sentinel(CAP, CAP)];
                nest_slots = vec![
                    [sentinel(NCAP, NCAP), sentinel(NCAP, NCAP)],
                    [sentinel(NCAP, NCAP), sentinel(NCAP, NCAP)],
                ];
                ecmwf_saved = SavedInit::default();
                gfs_saved = GfsSaved::new(CAP, CAP, KZ);
                zgrid = ZGrid {
                    height: vec![0.0; KZ],
                    nmixz: u(r.outs[3]),
                };
            }
            "vt.grid" => {
                grids.insert(u(r.args[0]), parse_grid(&r.outs));
            }
            "vt.in" => {
                sets.entry(u(r.args[0]))
                    .or_default()
                    .insert((u(r.args[1]), u(r.args[2])), r.outs.clone());
            }
            "vt.nset" => {
                let o = &r.outs;
                nsets.insert(
                    u(r.args[0]),
                    NestDef {
                        l: u(r.args[1]),
                        geom: GridGeometry {
                            nx: u(o[0]),
                            ny: u(o[1]),
                            dx: o[2],
                            dy: o[3],
                            xlon0: o[4],
                            ylat0: o[5],
                        },
                        xresoln: o[6],
                        yresoln: o[7],
                    },
                );
            }
            "vt.call" => {
                let a = &r.args;
                let (proc, call, routine, gid, sid, n) =
                    (u(a[0]), u(a[1]), u(a[2]), u(a[3]), u(a[4]), u(a[5]));
                let slot = n - 1;
                let g = grids[&gid].clone();
                match routine {
                    1 | 2 => {
                        let clouds = CloudScheme {
                            readclouds: a[7] != 0.0,
                            sumclouds: a[9] != 0.0,
                        };
                        let raw = raw_fields(&sets[&sid], g.grid.geom.nx, g.grid.geom.ny);
                        let res = if routine == 1 {
                            verttransform_ecmwf(
                                &mut ecmwf_saved,
                                &mut zgrid,
                                &g.grid,
                                &g.hyb,
                                &g.polar,
                                clouds,
                                &raw,
                                &mut slots[slot],
                            )
                        } else {
                            verttransform_gfs(
                                &mut gfs_saved,
                                &mut zgrid,
                                &g.grid,
                                &g.hyb,
                                &g.polar,
                                clouds,
                                &raw,
                                &mut slots[slot],
                            )
                        };
                        res.unwrap_or_else(|e| panic!("call {proc}/{call}: port refused: {e:?}"));
                        snaps.insert(
                            (proc, call),
                            Snapshot::Mother {
                                slot: slots[slot].clone(),
                                zgrid: zgrid.clone(),
                            },
                        );
                    }
                    3 => {
                        let sid2 = u(a[6]);
                        let flags = [(a[7], a[9]), (a[8], a[10])];
                        let nests: Vec<NestInput> = [sid, sid2]
                            .iter()
                            .enumerate()
                            .map(|(i, s)| {
                                let nd = &nsets[s];
                                assert_eq!(nd.l, i + 1);
                                NestInput {
                                    grid: VertGrid {
                                        geom: nd.geom,
                                        ..g.grid
                                    },
                                    xresoln: nd.xresoln,
                                    yresoln: nd.yresoln,
                                    clouds: CloudScheme {
                                        readclouds: flags[i].0 != 0.0,
                                        sumclouds: flags[i].1 != 0.0,
                                    },
                                    raw: raw_fields(&sets[s], nd.geom.nx, nd.geom.ny),
                                }
                            })
                            .collect();
                        let mut outs: Vec<ZFields> =
                            nest_slots.iter().map(|s| s[slot].clone()).collect();
                        verttransform_nests(&zgrid.height, &g.hyb, &nests, &mut outs)
                            .unwrap_or_else(|e| panic!("call {proc}/{call}: port refused: {e:?}"));
                        for (l, o) in outs.iter().enumerate() {
                            nest_slots[l][slot] = o.clone();
                        }
                        snaps.insert((proc, call), Snapshot::Nests { nests: outs });
                    }
                    _ => panic!("unknown routine {routine}"),
                }
            }
            _ => {}
        }
    }
    Replay { snaps }
}

fn replays() -> &'static [Replay; 2] {
    static R: OnceLock<[Replay; 2]> = OnceLock::new();
    R.get_or_init(|| {
        let f = fixtures();
        [replay(&f.real4), replay(&f.real8)]
    })
}

fn replay_for(p: Precision) -> &'static Replay {
    match p {
        Precision::Real4 => &replays()[0],
        Precision::Real8 => &replays()[1],
    }
}

fn levels(f: &Field3, ix: usize, jy: usize) -> impl Iterator<Item = f64> + '_ {
    (0..KZ).map(move |k| f.at(ix, jy, k))
}

fn out_row(z: &ZFields, ix: usize, jy: usize) -> Vec<f64> {
    let mut v = Vec::with_capacity(12 * KZ);
    for f in [
        &z.uu, &z.vv, &z.ww, &z.tt, &z.qv, &z.pv, &z.rho, &z.drhodz, &z.prs, &z.pplev, &z.uupol,
        &z.vvpol,
    ] {
        v.extend(levels(f, ix, jy));
    }
    v
}

fn nest_out_row(z: &ZFields, ix: usize, jy: usize) -> Vec<f64> {
    let mut v = Vec::with_capacity(8 * KZ);
    for f in [&z.uu, &z.vv, &z.ww, &z.tt, &z.qv, &z.pv, &z.rho, &z.drhodz] {
        v.extend(levels(f, ix, jy));
    }
    v
}

fn cloud_row(z: &ZFields, ix: usize, jy: usize) -> Vec<f64> {
    let mut v = Vec::with_capacity(4 * KZ + 2);
    for f in [&z.clwc, &z.ciwc, &z.clw] {
        v.extend(levels(f, ix, jy));
    }
    v.extend((0..KZ).map(|k| f64::from(z.cloud(ix, jy, k))));
    v.push(f64::from(z.cloud_height(ix, jy)));
    v.push(z.ctwc.at(ix, jy));
    v
}

fn mother(p: Precision, r: &Row) -> (&'static ZFields, &'static ZGrid) {
    match &replay_for(p).snaps[&(u(r.args[0]), u(r.args[1]))] {
        Snapshot::Mother { slot, zgrid } => (slot, zgrid),
        Snapshot::Nests { .. } => panic!("row {:?} is not a mother-grid call", r.args),
    }
}

fn nest(p: Precision, r: &Row) -> &'static ZFields {
    match &replay_for(p).snaps[&(u(r.args[0]), u(r.args[1]))] {
        Snapshot::Nests { nests } => &nests[u(r.args[2]) - 1],
        Snapshot::Mother { .. } => panic!("row {:?} is not a nest call", r.args),
    }
}

fn all(_: &Row, _: Precision) -> bool {
    true
}

/// Columns of an `*.out` row (12 fields x 8 levels; nests 8 fields) where a
/// real(4) miss may be attributed to upstream's single precision: `vv`
/// (8..16), `ww` (16..24), `qv` (32..40), `pv` (40..48), `drhodz` (56..64).
///
/// Diagnosis (measured on this fixture, every miss inside 4x upstream's own
/// real(4)-vs-real(8) distance, none unexplained):
///
/// * **The interpolation weights cancel.** `dz1 = height(iz) - uvzlev(kz-1)`
///   is a difference of two heights of a few hundred metres; where a column's
///   level lies within centimetres of a reference height (columns whose `ps`
///   is close to the reference column's), `dz1` keeps only a few significant
///   `f32` digits. The interpolated value's error is that relative error times
///   `(f_k - f_{k-1})/f`, which is large where the field changes a lot across
///   the layer relative to its value: `qv` between a wet and a dry level
///   (contrast ~100), `pv`, and `vv` where it crosses zero.
/// * **`ww` is a cancelling sum.** Eta-dot of opposite sign on adjacent
///   levels, and the slope term `dz/dx u + dz/dy v` of comparable size, sum to
///   `1e-5 .. 1e-3` m/s from terms ~10x larger; at the poles the zonal mean
///   of `ww` can cancel to `1e-17` (real(8)) against `1.5e-9` (real(4)).
/// * **`drhodz`** is a difference of interpolated densities over two layers.
///
/// Worst real(4) deviations: `ww` 2.7e-3 (ECMWF), 5.9e-3 (GFS, excluding the
/// pole cancellation to ~0), `qv` 1.1e-3, `vv` 2.5e-4, `drhodz` 2.2e-5.
const OUT_SPREAD: &[usize] = &[
    8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 32, 33, 34, 35, 36, 37, 38, 39,
    40, 41, 42, 43, 44, 45, 46, 47, 56, 57, 58, 59, 60, 61, 62, 63,
];

/// Columns of a `*.cloud` row where the spread rule applies: `clwc` (0..8),
/// `ciwc` (8..16), `clw` (16..24). Same mechanism as `OUT_SPREAD`, in its
/// purest form: cloud water sits on one raw level and is 0 on the next, so
/// the interpolated value is `cw * dz1/dz` and inherits the full relative
/// error of a cancelling `dz1` (8.2e-3 at worst: `dz1/dz = 7e-5`, i.e.
/// `dz1 ~ 0.09 m` from heights of 269 m, whose `f32` spacing is 3e-5 m).
/// The integer outputs (`clouds`, `cloudsh`) and `ctwc` are held strictly,
/// and agree exactly: no cloud code or truncated cloud height flips.
const CLOUD_SPREAD: &[usize] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

/// Real(8): bit-exact expected (bound 1e-13). Real(4): 1e-5, with the
/// spread rule on the diagnosed columns.
const OUT_BOUNDS: Bounds = Bounds {
    tol8: 1e-13,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(OUT_SPREAD),
};

/// As `OUT_BOUNDS` for the cloud rows.
const CLOUD_BOUNDS: Bounds = Bounds {
    tol8: 1e-13,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(CLOUD_SPREAD),
};

/// Heights and `nmixz`: strict at both precisions.
const TIGHT: Bounds = Bounds::rel(1e-13, 1e-5);

fn eval_out(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let (z, _) = mother(p, r);
    Some(out_row(z, u(r.args[2]), u(r.args[3])))
}

fn eval_cloud(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let (z, _) = mother(p, r);
    Some(cloud_row(z, u(r.args[2]), u(r.args[3])))
}

fn eval_zgrid(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let (_, zg) = mother(p, r);
    let mut v = zg.height[..KZ].to_vec();
    v.push(zg.nmixz as f64);
    Some(v)
}

/// `verttransform_ecmwf`: winds, `ww`, T, q, PV, `rho`, `drhodz`, `prs`,
/// `uupol`/`vvpol` on every level of every column after every call.
#[test]
fn ecmwf_fields() {
    check_group(fixtures(), "ecmwf.out", OUT_BOUNDS, all, eval_out);
}

/// `verttransform_ecmwf`: cloud water, `clw`, `clouds`, `cloudsh`, `ctwc`.
#[test]
fn ecmwf_clouds() {
    check_group(fixtures(), "ecmwf.cloud", CLOUD_BOUNDS, all, eval_cloud);
}

/// `height` and `nmixz` after every ECMWF call.
#[test]
fn ecmwf_zgrid() {
    check_group(fixtures(), "ecmwf.zgrid", TIGHT, all, eval_zgrid);
}

/// `verttransform_gfs` fields (`pplev` instead of `prs`).
#[test]
fn gfs_fields() {
    check_group(fixtures(), "gfs.out", OUT_BOUNDS, all, eval_out);
}

/// `verttransform_gfs` clouds.
#[test]
fn gfs_clouds() {
    check_group(fixtures(), "gfs.cloud", CLOUD_BOUNDS, all, eval_cloud);
}

/// `height` and `nmixz` after every GFS call.
#[test]
fn gfs_zgrid() {
    check_group(fixtures(), "gfs.zgrid", TIGHT, all, eval_zgrid);
}

/// `verttransform_nests` fields, both nests.
#[test]
fn nests_fields() {
    check_group(fixtures(), "nests.out", OUT_BOUNDS, all, |r, p| {
        Some(nest_out_row(nest(p, r), u(r.args[3]), u(r.args[4])))
    });
}

/// `verttransform_nests` clouds, both nests.
#[test]
fn nests_clouds() {
    check_group(fixtures(), "nests.cloud", CLOUD_BOUNDS, all, |r, p| {
        Some(cloud_row(nest(p, r), u(r.args[3]), u(r.args[4])))
    });
}

/// `shift_field_0`: a permutation, exact at both precisions.
#[test]
fn shift_field_0_matches() {
    check_group(
        fixtures(),
        "shift_field_0",
        Bounds::rel(0.0, 0.0),
        all,
        |r, _| {
            let (s, nxf, nyf) = (r.args[0] as i64, u(r.args[1]), u(r.args[2]));
            let ldx = nxf + 1;
            let ldy = nyf + 1;
            let mut f = vec![0.0; ldx * ldy];
            for jy in 0..ldy {
                for ix in 0..ldx {
                    f[ix + ldx * jy] = (100 * jy + ix) as f64;
                }
            }
            shift_field_0(&mut f, ldx, ldy, nxf, nyf, s).unwrap();
            Some(f)
        },
    );
}

/// `shift_field` on slot 2 of a 2-slot, 3-level field with `nzf = 2`:
/// slot 1 and level 3 untouched.
#[test]
fn shift_field_matches() {
    check_group(
        fixtures(),
        "shift_field",
        Bounds::rel(0.0, 0.0),
        all,
        |r, _| {
            let (s, nxf, nyf) = (r.args[0] as i64, u(r.args[1]), u(r.args[2]));
            let (nzfmax, nzf, nmax, n) = (u(r.args[3]), u(r.args[4]), u(r.args[5]), u(r.args[6]));
            let ldx = nxf + 1;
            let ldy = nyf + 1;
            let per = ldx * ldy * nzfmax;
            let mut f = vec![0.0; per * nmax];
            for m in 0..nmax {
                for kz in 0..nzfmax {
                    for jy in 0..ldy {
                        for ix in 0..ldx {
                            f[ix + ldx * (jy + ldy * kz) + per * m] =
                                (1000 * (m + 1) + 100 * (kz + 1) + 10 * jy + ix) as f64;
                        }
                    }
                }
            }
            shift_field(&mut f[per * (n - 1)..per * n], ldx, ldy, nxf, nyf, nzf, s).unwrap();
            Some(f)
        },
    );
}
