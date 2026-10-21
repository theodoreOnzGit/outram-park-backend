// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::release` and
// `changi::flexpart::domainfill` against upstream FLEXPART v10.4 (commit
// 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART particle release and domain filling, code-to-code
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_release.f90` links `releaseparticles.f90`,
//! `init_domainfill.f90`, `boundcond_domainfill.f90`, `juldate.f90` and
//! `caldate.f90` **verbatim**. Two local files stand in for upstream ones, and
//! neither changes a routine under test:
//! - `par_mod.f90` is compiled from a copy with three configuration lines
//!   changed (one nest, `maxspec = 2`, `nclassunc = 5`; diff checked);
//! - `dev/random_mod_shim_release.f90` replaces the Numerical Recipes
//!   generators: `ran1` returns a driver-set sequence of dyadic values in
//!   `(0, 1)`, and every call's consumed draws are echoed and fed to the port.
//!
//! Synthetic fields are computed in double and rounded once to `real(4)` in
//! both builds. The driver makes 18 `releaseparticles` calls, 6
//! `init_domainfill` calls and 11 `boundcond_domainfill` calls, sweeping:
//! - **release**: point and area sources (species-dependent hour and
//!   day-of-week profiles, two species); release windows (before, at the
//!   start (half), inside, at the end (half), after, instantaneous);
//!   `xmasssave` carried between calls; forward and backward; summer-time
//!   and winter months; `nhour = 0 -> 24` with `ndayofweek 0 -> 7` (Monday
//!   midnight local); `xlonav` wrapped both ways; `kindz` 1, 2 and 3 with all
//!   three outcomes of the pressure search; the `eps2` and top clamps; a nest;
//!   `ind_rel` 0, 1, 3; `mquasilag`; `DRYBKDEP`, `WETBKDEP`; the east-west
//!   wrap of a global grid both ways; reads at `x == nxmin1`, `y == nymin1`;
//!   occupied and terminated slots between vacant ones;
//! - **init_domainfill**: a limited box (columns above and below 20
//!   particles), the ozone tracer across the equator with the box at the east
//!   and north grid edges (extra x draw, PV from row `ny`), a resume
//!   (`ipin = 2`) with particles outside the domain and a trailing invalid
//!   one, a global box (`gdomainfill`, polar caps, `ix == 0` draw), a global
//!   resume (early return), and global winds with a box short of the poles;
//! - **boundcond_domainfill**: inflow and outflow on all four sides, several
//!   particles per step, terminations (west, east, north, south, and on the
//!   boundary itself), the ozone rejection path, the global early return,
//!   the `xglobal` no-x-termination branch, and a synthetic state with
//!   release-height counts 0, 1, 3, 4, 5.
//!
//! The particle store (100 000 slots) and the boundary arrays (upstream's
//! extents, column-major) are reconstructed from the fixture: the driver
//! dumps every slot and array element a call or the driver itself changed.
//! Each port call starts from the Fortran's own pre-call state, so a
//! discrepancy is local to one call. The test compares every changed value,
//! and separately fails if the port changed anything the Fortran did not.
//!
//! ## Results
//!
//! Re-print with `cargo test --release -p changi --test
//! flexpart_release_code_to_code -- --nocapture`. Taken 2026-10-02, upstream
//! `3d7eebf`:
//!
//! | Group | Rows (r8 / r4 in scope) | vs real(8) | vs real(4) |
//! |---|---:|---:|---:|
//! | `rel_slot` (released particles) | 235 / 235 | 0 (3055/3055 bit-exact) | 5.3e-7 |
//! | `rel_after` (counters, `xmasssave`, `rho_rel`) | 18 / 18 | 0 (bit-exact) | 5.8e-7 |
//! | `df_slot` (domain-fill particles) | 1687 / 771 | 0 (bit-exact) | 6.5e-5 (70 spread) |
//! | `df_cell` heights and counts | 244 / 200 | 0 (bit-exact) | 6.5e-6 |
//! | `df_cell` accumulators | 315 / 218 | 0 (bit-exact) | 1.1e-4 (1 spread) |
//! | `df_after` (box, counts, `xmassperparticle`) | 17 / 12 | 0 (bit-exact) | 9.3e-6 |
//!
//! Every real(8) value is bit-identical. Real(4):
//! - **Out of scope: calls 27-35** (the first domain-filling box and its four
//!   boundary steps). Upstream's own two builds disagree there: column
//!   (4, 4) gets `nint(16.4995...)` particles, and the real(4) cell area,
//!   computed as `sqrt(r^2 - c_p^2) - sqrt(r^2 - c_m^2)` with `r^2 ~ 4e13` in
//!   24 bits, is 6.5e-5 off (measured by emulating the f32 arithmetic), enough
//!   to round it to 17. The shipped build therefore has 500 particles where
//!   the real(8) build (and the port) has 499. See [`diverged`].
//! - **Spread-attributed**: `xmass1` and `ztra1` of domain-filling particles
//!   (the same cell-area cancellation, up to 6.5e-5; and `pp(kz) - pnew`
//!   near a level, up to 2.0e-5), `xmassperparticle`, and one accumulator
//!   (`acc - mmass * xmassperparticle`, a remainder 2.8e3 times smaller than
//!   its terms: 1.1e-4). In each the port agrees with the real(8) build to
//!   every printed digit.
//!
//! ## What this does NOT establish
//!
//! The file I/O branches (`ipin = 1` with a limited box, `ipout`) are not
//! ported (the port reports them); upstream's `stop`s and out-of-bounds reads
//! are refusals checked by unit tests, not by the fixture. Nothing here
//! supports emergency response or dose assessment for real populations.

mod common;

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use changi::flexpart::advance::NestGeometry;
use changi::flexpart::domainfill::{
    boundcond_domainfill, init_domainfill, DomainFillGrid, DomainFillMode, DomainFillSettings,
    DomainFillState,
};
use changi::flexpart::interpolation::MetFields;
use changi::flexpart::release::{
    releaseparticles, EmissionVariation, KindZ, ParticleStore, ReleaseDomain, ReleaseNest,
    ReleasePoint, ReleaseSettings, StaticExtents, ITRA_INACTIVE,
};
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_release_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_release_real8.csv");

const MAXPART: usize = 100_000;
const NSPEC: usize = 2;
const EXTENTS: StaticExtents = StaticExtents::SHIPPED;
const MAXCOLUMN: usize = 3000;
const DP: &[&str] = &[
    "grid",
    "fld",
    "oro",
    "nest",
    "nfld",
    "noro",
    "profile",
    "reset",
    "pset",
    "clock",
    "rel_cfg",
    "rel_point",
    "rel_call",
    "rel_slot",
    "rel_after",
    "draws",
    "df_fill",
    "df_set",
    "df_cfg",
    "df_init_call",
    "df_bc_call",
    "df_slot",
    "df_cell",
    "df_after",
];

fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, DP))
}

fn key(r: &Row) -> String {
    format!("{}|{:?}", r.function, r.args)
}

/// One grid of the driver.
#[derive(Clone)]
struct Grid {
    met: MetFields,
    pv: Vec<f64>,
    oro: Vec<f64>,
    nx: i64,
    ny: i64,
    dx: f64,
    dy: f64,
    xlon0: f64,
    ylat0: f64,
    dyconst: f64,
    xglobal: bool,
    sglobal: bool,
    nglobal: bool,
    numbnests: usize,
}

/// What the replay of one fixture produced.
struct Replay {
    /// Port outputs by row key.
    outs: HashMap<String, Vec<f64>>,
    /// Values the port changed that the Fortran did not, and port refusals.
    problems: Vec<String>,
}

fn slot_values(p: &ParticleStore, s: usize) -> Vec<f64> {
    vec![
        p.itra1[s] as f64,
        p.npoint[s] as f64,
        p.nclass[s] as f64,
        p.idt[s] as f64,
        p.itramem[s] as f64,
        p.itrasplit[s] as f64,
        p.xtra1[s],
        p.ytra1[s],
        p.ztra1[s],
        p.xmass1[s * NSPEC],
        p.xmass1[s * NSPEC + 1],
        p.xscav_frac1[s * NSPEC],
        p.xscav_frac1[s * NSPEC + 1],
    ]
}

fn set_slot(p: &mut ParticleStore, s: usize, v: &[f64]) {
    p.itra1[s] = v[0] as i64;
    p.npoint[s] = v[1] as i64;
    p.nclass[s] = v[2] as i64;
    p.idt[s] = v[3] as i64;
    p.itramem[s] = v[4] as i64;
    p.itrasplit[s] = v[5] as i64;
    p.xtra1[s] = v[6];
    p.ytra1[s] = v[7];
    p.ztra1[s] = v[8];
    p.xmass1[s * NSPEC] = v[9];
    p.xmass1[s * NSPEC + 1] = v[10];
    p.xscav_frac1[s * NSPEC] = v[11];
    p.xscav_frac1[s * NSPEC + 1] = v[12];
}

/// Element `(id, k, i, j)` (upstream's 1-based `k`, `j`) of the boundary
/// arrays: `(array, linear index)`.
fn cell_index(st: &DomainFillState, id: i64, k: i64, i: i64, j: i64) -> (i64, usize) {
    let k0 = (k - 1) as usize;
    match id {
        1 | 2 => (id, k0 + 2 * i as usize),
        3 | 5 => (id, st.we(k0, i, j - 1).expect("we index")),
        4 | 6 => (id, st.sn(k0, i, j - 1).expect("sn index")),
        _ => panic!("cell id {id}"),
    }
}

fn get_cell(st: &DomainFillState, id: i64, l: usize) -> f64 {
    match id {
        1 => st.numcolumn_we[l] as f64,
        2 => st.numcolumn_sn[l] as f64,
        3 => st.zcolumn_we[l],
        4 => st.zcolumn_sn[l],
        5 => st.acc_mass_we[l],
        _ => st.acc_mass_sn[l],
    }
}

fn set_cell(st: &mut DomainFillState, id: i64, l: usize, v: f64) {
    match id {
        1 => st.numcolumn_we[l] = v as i64,
        2 => st.numcolumn_sn[l] = v as i64,
        3 => st.zcolumn_we[l] = v,
        4 => st.zcolumn_sn[l] = v,
        5 => st.acc_mass_we[l] = v,
        _ => st.acc_mass_sn[l] = v,
    }
}

/// Every element of the six boundary arrays that differs between `a` and `b`.
fn changed_cells(a: &DomainFillState, b: &DomainFillState) -> Vec<(i64, usize)> {
    let mut out = Vec::new();
    for (i, (x, y)) in a.numcolumn_we.iter().zip(&b.numcolumn_we).enumerate() {
        if x != y {
            out.push((1, i));
        }
    }
    for (i, (x, y)) in a.numcolumn_sn.iter().zip(&b.numcolumn_sn).enumerate() {
        if x != y {
            out.push((2, i));
        }
    }
    for (id, va, vb) in [
        (3, &a.zcolumn_we, &b.zcolumn_we),
        (4, &a.zcolumn_sn, &b.zcolumn_sn),
        (5, &a.acc_mass_we, &b.acc_mass_we),
        (6, &a.acc_mass_sn, &b.acc_mass_sn),
    ] {
        for (i, (x, y)) in va.iter().zip(vb.iter()).enumerate() {
            if x.to_bits() != y.to_bits() {
                out.push((id, i));
            }
        }
    }
    out
}

fn apply_cells(st: &mut DomainFillState, rows: &[&Row]) {
    for r in rows {
        let (id, k, i, j0) = (
            r.args[1] as i64,
            r.args[2] as i64,
            r.args[3] as i64,
            r.args[4] as i64,
        );
        for (n, &v) in r.outs.iter().enumerate() {
            let (id, l) = cell_index(st, id, k, i, j0 + n as i64);
            set_cell(st, id, l, v);
        }
    }
}

fn clock(p: &mut ParticleStore, a: &[f64]) {
    let itime = a[1] as i64;
    let rule = a[2] as i64;
    let (we0, we1, sn0, sn1) = (a[3], a[4], a[5], a[6]);
    for ip in 1..=p.numpart {
        let s = ip - 1;
        if p.itra1[s] == ITRA_INACTIVE {
            continue;
        }
        p.itra1[s] = itime;
        if rule == 1 {
            if ip % 7 == 3 {
                p.itra1[s] = ITRA_INACTIVE;
            }
        } else {
            if ip % 11 == 4 {
                p.xtra1[s] = we0 - 0.25;
            }
            if ip % 13 == 6 {
                p.ytra1[s] = sn1 + 0.25;
            }
            if ip % 17 == 2 {
                p.xtra1[s] = we1 + 0.5;
            }
            if ip % 19 == 1 {
                p.xtra1[s] = we0;
            }
            if ip % 23 == 5 {
                p.ytra1[s] = sn0 - 0.125;
            }
        }
    }
}

/// Compare the port's particle store with the Fortran's post-call one on
/// every slot the port changed; report slots the Fortran left alone.
fn check_extra_slots(
    name: &str,
    seq: u64,
    pre: &ParticleStore,
    port: &ParticleStore,
    dumped: &HashSet<usize>,
    problems: &mut Vec<String>,
) {
    for s in 0..MAXPART {
        let a = slot_values(pre, s);
        let b = slot_values(port, s);
        let changed = a.iter().zip(&b).any(|(x, y)| x.to_bits() != y.to_bits());
        if changed && !dumped.contains(&(s + 1)) {
            problems.push(format!(
                "{name} seq {seq}: port changed slot {} (Fortran did not)",
                s + 1
            ));
        }
    }
}

#[allow(clippy::too_many_lines)]
fn replay(p: Precision) -> Replay {
    let rows = fixtures().rows(p);
    // Index the rows that follow a call by sequence number.
    let mut draws: HashMap<u64, Vec<f64>> = HashMap::new();
    let mut dumps: HashMap<(String, u64), Vec<&Row>> = HashMap::new();
    for r in rows {
        match r.function.as_str() {
            "draws" => {
                draws.insert(r.args[0] as u64, r.outs.clone());
            }
            "rel_slot" | "df_slot" | "df_cell" | "df_set" | "pset" => {
                dumps
                    .entry((r.function.clone(), r.args[0] as u64))
                    .or_default()
                    .push(r);
            }
            _ => {}
        }
    }

    let mut outs = HashMap::new();
    let mut problems = Vec::new();
    let mut grids: HashMap<i64, Grid> = HashMap::new();
    let mut gid = 0i64;
    let mut nest: Option<ReleaseNest> = None;
    let mut variation = EmissionVariation::uniform(NSPEC);
    let mut store = ParticleStore::new(MAXPART, NSPEC);
    let mut state = DomainFillState::new(EXTENTS.nxmax, EXTENTS.nymax, MAXCOLUMN);
    let mut points: Vec<ReleasePoint> = Vec::new();
    let mut rel_cfg: Vec<f64> = Vec::new();
    let mut df_cfg: Vec<f64> = Vec::new();
    let mut last_after: HashMap<&str, Vec<f64>> = HashMap::new();

    for (n, r) in rows.iter().enumerate() {
        let a = &r.args;
        match r.function.as_str() {
            "grid" => {
                let (nx, ny, nz) = (a[1] as usize, a[2] as usize, a[3] as usize);
                let mut met = MetFields::zeros(nx, ny, nz, 1);
                met.height = a[13..13 + nz].to_vec();
                met.memtime = [0, 10800];
                met.memind = [0, 1];
                gid = a[0] as i64;
                grids.insert(
                    gid,
                    Grid {
                        pv: vec![0.0; met.uu.len()],
                        oro: vec![0.0; nx * ny],
                        met,
                        nx: nx as i64,
                        ny: ny as i64,
                        dx: a[4],
                        dy: a[5],
                        xlon0: a[6],
                        ylat0: a[7],
                        dyconst: a[8],
                        xglobal: a[9] != 0.0,
                        sglobal: a[10] != 0.0,
                        nglobal: a[11] != 0.0,
                        numbnests: a[12] as usize,
                    },
                );
            }
            "fld" => {
                let g = grids.get_mut(&(a[0] as i64)).expect("grid");
                let (id, slot, k) = (a[1] as i64, a[2] as usize - 1, a[3] as usize - 1);
                let nx = g.met.nx;
                for (c, &v) in r.outs.iter().enumerate() {
                    let i = g.met.idx3(c % nx, c / nx, k, slot);
                    match id {
                        1 => g.met.uu[i] = v,
                        2 => g.met.vv[i] = v,
                        6 => g.met.rho[i] = v,
                        8 => g.met.tt[i] = v,
                        9 => g.pv[i] = v,
                        _ => panic!("field {id}"),
                    }
                }
            }
            "oro" => grids.get_mut(&(a[0] as i64)).expect("grid").oro = r.outs.clone(),
            "nest" => {
                let mut met = MetFields::zeros(a[6] as usize, a[7] as usize, grids[&gid].met.nz, 1);
                met.height = grids[&gid].met.height.clone();
                nest = Some(ReleaseNest {
                    geometry: NestGeometry {
                        xln: a[0],
                        yln: a[1],
                        xrn: a[2],
                        yrn: a[3],
                        xresoln: a[4],
                        yresoln: a[5],
                    },
                    oro: vec![0.0; met.nx * met.ny],
                    met,
                });
            }
            "nfld" => {
                let m = &mut nest.as_mut().expect("nest").met;
                let (id, k) = (a[0] as i64, a[1] as usize - 1);
                let nx = m.nx;
                for (c, &v) in r.outs.iter().enumerate() {
                    let i = m.idx3(c % nx, c / nx, k, 1);
                    if id == 6 {
                        m.rho[i] = v;
                    } else {
                        m.tt[i] = v;
                    }
                }
            }
            "noro" => nest.as_mut().expect("nest").oro = r.outs.clone(),
            "profile" => {
                let k = a[0] as usize - 1;
                let o = &r.outs;
                variation.area_hour[k * 24..k * 24 + 24].copy_from_slice(&o[0..24]);
                variation.point_hour[k * 24..k * 24 + 24].copy_from_slice(&o[24..48]);
                variation.area_dow[k * 7..k * 7 + 7].copy_from_slice(&o[48..55]);
                variation.point_dow[k * 7..k * 7 + 7].copy_from_slice(&o[55..62]);
            }
            "reset" => store = ParticleStore::new(MAXPART, NSPEC),
            "pset" => {
                set_slot(&mut store, a[1] as usize - 1, &r.outs);
            }
            "clock" => clock(&mut store, a),
            "rel_cfg" => rel_cfg = a.clone(),
            "rel_point" => {
                let i = a[0] as usize;
                points.truncate(i - 1);
                points.push(ReleasePoint {
                    ireleasestart: a[1] as i64,
                    ireleaseend: a[2] as i64,
                    npart: a[3] as i64,
                    kindz: match a[4] as i64 {
                        1 => KindZ::AboveGround,
                        2 => KindZ::AboveSeaLevel,
                        _ => KindZ::Pressure,
                    },
                    xpoint1: a[5],
                    xpoint2: a[6],
                    ypoint1: a[7],
                    ypoint2: a[8],
                    zpoint1: a[9],
                    zpoint2: a[10],
                    xmass: vec![a[11], a[12]],
                });
            }
            "rel_call" => {
                let seq = a[0] as u64;
                let itime = a[1] as i64;
                let np = points.len();
                let g = &grids[&gid];
                let c = &rel_cfg;
                let ind_rel = c[6] as i64;
                let settings = ReleaseSettings {
                    ldirect: c[0] as i64,
                    lsynctime: c[1] as i64,
                    mintime: c[2] as i64,
                    itsplit: c[3] as i64,
                    nclassunc: c[4] as i64,
                    mquasilag: c[5] != 0.0,
                    density_weighted: matches!(ind_rel, 1 | 3 | 4),
                    backward_deposition: c[7] != 0.0 || c[8] != 0.0,
                };
                assert_eq!(c[12] as usize, EXTENTS.nxmax, "nxmax");
                let domain = ReleaseDomain {
                    nxmin1: g.nx - 1,
                    xglobal: g.xglobal,
                    xlon0: g.xlon0,
                    dx: g.dx,
                    bdate: c[9],
                    extents: EXTENTS,
                };
                let nests: Vec<ReleaseNest> = if g.numbnests == 1 {
                    vec![nest.clone().expect("nest")]
                } else {
                    Vec::new()
                };
                let mut parts = store.clone();
                parts.numpart = a[3] as usize;
                parts.numparticlecount = a[4] as i64;
                let mut xmasssave = a[5..5 + np].to_vec();
                let mut rho_rel = a[5 + np..5 + 2 * np].to_vec();
                let d = draws.get(&seq).cloned().unwrap_or_default();
                let mut it = d.iter().copied();
                if let Err(e) = releaseparticles(
                    itime,
                    &points,
                    &mut xmasssave,
                    &mut rho_rel,
                    &variation,
                    &settings,
                    &domain,
                    &g.met,
                    &g.oro,
                    &nests,
                    &mut it,
                    &mut parts,
                ) {
                    problems.push(format!("releaseparticles seq {seq}: port refused: {e:?}"));
                }
                let consumed = d.len() - it.len();
                let empty = Vec::new();
                let dumped = dumps.get(&("rel_slot".into(), seq)).unwrap_or(&empty);
                let set: HashSet<usize> = dumped.iter().map(|r| r.args[1] as usize).collect();
                check_extra_slots("releaseparticles", seq, &store, &parts, &set, &mut problems);
                for dr in dumped {
                    outs.insert(key(dr), slot_values(&parts, dr.args[1] as usize - 1));
                    set_slot(&mut store, dr.args[1] as usize - 1, &dr.outs);
                }
                let mut o = vec![
                    parts.numpart as f64,
                    parts.numparticlecount as f64,
                    consumed as f64,
                ];
                o.extend_from_slice(&xmasssave);
                o.extend_from_slice(&rho_rel);
                last_after.insert("rel_after", o);
            }
            "rel_after" => {
                outs.insert(key(r), last_after.remove("rel_after").expect("rel_after"));
                store.numpart = r.outs[0] as usize;
                store.numparticlecount = r.outs[1] as i64;
            }
            "df_fill" => {
                let (ncol, z, acc) = (a[0] as i64, a[1], a[2]);
                let (imax, jmax) = (a[3] as i64, a[4] as i64);
                for i in 0..=imax {
                    for k in 0..2usize {
                        state.numcolumn_we[k + 2 * i as usize] = ncol;
                        state.numcolumn_sn[k + 2 * i as usize] = ncol;
                        for j in 0..jmax {
                            let l = state.we(k, i, j).expect("we");
                            state.zcolumn_we[l] = z;
                            state.acc_mass_we[l] = acc;
                            let l = state.sn(k, i, j).expect("sn");
                            state.zcolumn_sn[l] = z;
                            state.acc_mass_sn[l] = acc;
                        }
                    }
                }
            }
            "df_set" => apply_cells(&mut state, &[r]),
            "df_cfg" => df_cfg = a.clone(),
            "df_init_call" | "df_bc_call" => {
                let seq = a[0] as u64;
                let g = &grids[&gid];
                let c = &df_cfg;
                let settings = DomainFillSettings {
                    mode: if c[0] as i64 == 1 {
                        DomainFillMode::Air
                    } else {
                        DomainFillMode::StratosphericOzone
                    },
                    ipin: c[1] as i64,
                    ipout: c[2] as i64,
                    pvcrit: c[3],
                    ozonescale: c[4],
                    nclassunc: c[5] as i64,
                    mintime: c[6] as i64,
                    ldirect: c[7] as i64,
                    itsplit: c[8] as i64,
                    lsynctime: c[9] as i64,
                };
                let grid = DomainFillGrid {
                    nx: g.nx,
                    ny: g.ny,
                    xglobal: g.xglobal,
                    sglobal: g.sglobal,
                    nglobal: g.nglobal,
                    dx: g.dx,
                    dy: g.dy,
                    ylat0: g.ylat0,
                    dyconst: g.dyconst,
                    extents: EXTENTS,
                };
                let mut parts = store.clone();
                let mut st = state.clone();
                let d = draws.get(&seq).cloned().unwrap_or_default();
                let mut it = d.iter().copied();
                let name = if r.function == "df_init_call" {
                    "init_domainfill"
                } else {
                    "boundcond_domainfill"
                };
                let res = if r.function == "df_init_call" {
                    // seq;pos0;numpart;numparticlecount;gdomainfill;numcolumn
                    parts.numpart = a[2] as usize;
                    parts.numparticlecount = a[3] as i64;
                    st.gdomainfill = a[4] != 0.0;
                    st.numcolumn = a[5] as i64;
                    init_domainfill(
                        c[10],
                        c[11],
                        c[12],
                        c[13],
                        c[14] as i64,
                        &grid,
                        &settings,
                        &g.met,
                        &g.pv,
                        &mut it,
                        &mut parts,
                        &mut st,
                    )
                    .map(|_| ())
                } else {
                    // seq;itime;pos0;numpart;numparticlecount;xmpp;nx_we;ny_sn;gdf;numcolumn;memtime
                    parts.numpart = a[3] as usize;
                    parts.numparticlecount = a[4] as i64;
                    st.xmassperparticle = a[5];
                    st.nx_we = [a[6] as i64, a[7] as i64];
                    st.ny_sn = [a[8] as i64, a[9] as i64];
                    st.gdomainfill = a[10] != 0.0;
                    st.numcolumn = a[11] as i64;
                    let mut met = g.met.clone();
                    met.memtime = [a[12] as i64, a[13] as i64];
                    boundcond_domainfill(
                        a[1] as i64,
                        -1,
                        &grid,
                        &settings,
                        &met,
                        &g.pv,
                        &mut it,
                        &mut parts,
                        &mut st,
                    )
                    .map(|_| ())
                };
                if let Err(e) = res {
                    problems.push(format!("{name} seq {seq}: port refused: {e:?}"));
                }
                let consumed = d.len() - it.len();
                // Particles.
                let empty = Vec::new();
                let dumped = dumps.get(&("df_slot".into(), seq)).unwrap_or(&empty);
                let set: HashSet<usize> = dumped.iter().map(|r| r.args[1] as usize).collect();
                check_extra_slots(name, seq, &store, &parts, &set, &mut problems);
                for dr in dumped {
                    outs.insert(key(dr), slot_values(&parts, dr.args[1] as usize - 1));
                    set_slot(&mut store, dr.args[1] as usize - 1, &dr.outs);
                }
                // Boundary arrays.
                let cells = dumps.get(&("df_cell".into(), seq)).unwrap_or(&empty);
                let mut covered = HashSet::new();
                for cr in cells {
                    let (id, k, i, j0) = (
                        cr.args[1] as i64,
                        cr.args[2] as i64,
                        cr.args[3] as i64,
                        cr.args[4] as i64,
                    );
                    let mut o = Vec::new();
                    for n in 0..cr.outs.len() {
                        let (id, l) = cell_index(&st, id, k, i, j0 + n as i64);
                        covered.insert((id, l));
                        o.push(get_cell(&st, id, l));
                    }
                    outs.insert(key(cr), o);
                }
                for c in changed_cells(&state, &st) {
                    if !covered.contains(&c) {
                        problems.push(format!(
                            "{name} seq {seq}: port changed boundary array {} element {} (Fortran did not)",
                            c.0, c.1
                        ));
                    }
                }
                apply_cells(&mut state, cells);
                last_after.insert(
                    "df_after",
                    vec![
                        st.nx_we[0] as f64,
                        st.nx_we[1] as f64,
                        st.ny_sn[0] as f64,
                        st.ny_sn[1] as f64,
                        st.numcolumn as f64,
                        if st.gdomainfill { 1.0 } else { 0.0 },
                        st.xmassperparticle,
                        parts.numpart as f64,
                        parts.numparticlecount as f64,
                        consumed as f64,
                    ],
                );
            }
            "df_after" => {
                outs.insert(key(r), last_after.remove("df_after").expect("df_after"));
                let o = &r.outs;
                state.nx_we = [o[0] as i64, o[1] as i64];
                state.ny_sn = [o[2] as i64, o[3] as i64];
                state.numcolumn = o[4] as i64;
                state.gdomainfill = o[5] != 0.0;
                state.xmassperparticle = o[6];
                store.numpart = o[7] as usize;
                store.numparticlecount = o[8] as i64;
            }
            "rel_slot" | "df_slot" | "df_cell" | "draws" => {}
            other => panic!("row {n}: unknown function `{other}`"),
        }
    }
    Replay { outs, problems }
}

fn replayed(p: Precision) -> &'static Replay {
    static R4: OnceLock<Replay> = OnceLock::new();
    static R8: OnceLock<Replay> = OnceLock::new();
    match p {
        Precision::Real4 => R4.get_or_init(|| replay(Precision::Real4)),
        Precision::Real8 => R8.get_or_init(|| replay(Precision::Real8)),
    }
}

fn evaluate(r: &Row, p: Precision) -> Option<Vec<f64>> {
    replayed(p).outs.get(&key(r)).cloned()
}

/// Calls whose real(4) and real(8) **Fortran** builds made different
/// discrete decisions: a different set of changed slots or boundary
/// elements, different integer slot values, counters or draw counts (the
/// draw *values* are not compared: they depend on how many draws earlier
/// calls consumed, and each call is fed its own echoed draws). The
/// criterion uses the two fixtures only, never the port. The port computes
/// in `f64` and follows the real(8) decision, so such a call's real(4) rows
/// cannot agree; they are taken out of real(4) scope and counted. Real(8)
/// keeps every row.
fn diverged() -> &'static HashSet<u64> {
    static D: OnceLock<HashSet<u64>> = OnceLock::new();
    D.get_or_init(|| {
        let sig = |p: Precision| {
            let mut m: HashMap<u64, Vec<String>> = HashMap::new();
            for r in fixtures().rows(p) {
                let seq = r.args.first().copied().unwrap_or(0.0) as u64;
                let s = match r.function.as_str() {
                    "rel_slot" | "df_slot" => {
                        format!("{}|{:?}|{:?}", r.function, r.args, &r.outs[..6])
                    }
                    "df_cell" => format!("{}|{:?}|{}", r.function, r.args, r.outs.len()),
                    "rel_after" => format!("{:?}", &r.outs[..3]),
                    "df_after" => format!("{:?}{:?}", &r.outs[..6], &r.outs[7..]),
                    _ => continue,
                };
                m.entry(seq).or_default().push(s);
            }
            m
        };
        let (s4, s8) = (sig(Precision::Real4), sig(Precision::Real8));
        let seqs: HashSet<u64> = s4.keys().chain(s8.keys()).copied().collect();
        seqs.into_iter()
            .filter(|q| s4.get(q) != s8.get(q))
            .collect()
    })
}

fn seq_of(r: &Row) -> u64 {
    r.args[0] as u64
}

/// See [`diverged`].
fn in_scope(r: &Row, p: Precision) -> bool {
    p == Precision::Real8 || !diverged().contains(&seq_of(r))
}

/// The boundary accumulators (`acc_mass_*`, ids 5 and 6).
fn acc_cells(r: &Row, p: Precision) -> bool {
    in_scope(r, p) && r.args[1] >= 5.0
}

/// Release heights and counts (ids 1 to 4).
fn height_cells(r: &Row, p: Precision) -> bool {
    in_scope(r, p) && r.args[1] < 5.0
}

/// The port changed nothing the Fortran did not, and never refused.
#[test]
fn port_changes_only_what_fortran_changes() {
    for p in [Precision::Real8, Precision::Real4] {
        let pr = &replayed(p).problems;
        let pr: Vec<&String> = pr
            .iter()
            .filter(|m| {
                p == Precision::Real8
                    || !m
                        .split(" seq ")
                        .nth(1)
                        .and_then(|t| t.split(':').next())
                        .and_then(|t| t.parse::<u64>().ok())
                        .is_some_and(|q| diverged().contains(&q))
            })
            .collect();
        assert!(
            pr.is_empty(),
            "{}: {} problems; first:\n{}",
            p.label(),
            pr.len(),
            pr.iter()
                .take(10)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    let mut d: Vec<&u64> = diverged().iter().collect();
    d.sort();
    println!("real4 out of scope (Fortran real4/real8 discrete divergence): calls {d:?}");
}

#[test]
fn releaseparticles_slots() {
    check_group(
        fixtures(),
        "rel_slot",
        Bounds::rel(1e-13, 1e-5),
        in_scope,
        evaluate,
    );
}

#[test]
fn releaseparticles_counters() {
    check_group(
        fixtures(),
        "rel_after",
        Bounds::rel(1e-13, 1e-5),
        in_scope,
        evaluate,
    );
}

#[test]
fn domainfill_slots() {
    // Column 9 is `xmass1`: the column air mass divided by its particle count.
    // Upstream's real(4) cell area `hzone = sqrt(r^2 - c_p^2) - sqrt(r^2 -
    // c_m^2)` cancels catastrophically (r^2 ~ 4e13 holds 24 bits), measured
    // at up to 6.5e-5 relative near the equator. Column 8 is `ztra1`, which
    // init_domainfill interpolates with `dz1 = pp(kz) - pnew`: a difference of
    // two pressures ~1e5 Pa (f32 ulp 0.008 Pa) that is small just above a
    // model level, measured at up to 2.0e-5 relative (call 54, slot 11:
    // real(8) 14.4347867, port 14.4347867, real(4) 14.4350719). See the
    // module docs.
    let b = Bounds {
        rule: Real4Rule::PrecisionSpreadOn(&[8, 9]),
        ..Bounds::rel(1e-13, 1e-5)
    };
    check_group(fixtures(), "df_slot", b, in_scope, evaluate);
}

#[test]
fn domainfill_boundary_heights() {
    check_group(
        fixtures(),
        "df_cell",
        Bounds::rel(1e-13, 1e-5),
        height_cells,
        evaluate,
    );
}

#[test]
fn domainfill_boundary_accumulators() {
    // `acc - mmass * xmassperparticle` cancels in real(4): the result is a
    // small remainder of two terms near `xmassperparticle` (~4e12, f32 ulp
    // 5e5); see the module docs.
    let b = Bounds {
        rule: Real4Rule::PrecisionSpread,
        ..Bounds::rel(1e-13, 1e-5)
    };
    check_group(fixtures(), "df_cell", b, acc_cells, evaluate);
}

#[test]
fn domainfill_counters() {
    // Column 6 is `xmassperparticle`, the box air mass over the particle
    // count: the same real(4) cell-area cancellation as `domainfill_slots`.
    let b = Bounds {
        rule: Real4Rule::PrecisionSpreadOn(&[6]),
        ..Bounds::rel(1e-13, 1e-5)
    };
    check_group(fixtures(), "df_after", b, in_scope, evaluate);
}
