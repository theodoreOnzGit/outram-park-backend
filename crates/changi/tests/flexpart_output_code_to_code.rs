// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::{concentration,
// plume_trajectory, particle_average}` against upstream FLEXPART v10.4
// (commit 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage "output" — gridding, plume statistics, averaging
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_output.f90` links upstream's `conccalc`,
//! `drydepokernel`, `drydepokernel_nest`, `centerofmass`, `clustering`,
//! `plumetraj`, `mean_mod` and `partpos_average` verbatim (with `com_mod`,
//! `unc_mod`, `outg_mod`, `point_mod`, `distance`, `distance2`). It writes
//! synthetic particles, output grids and met fields straight into `com_mod`;
//! the met fields are exact dyadic functions of their indices, rebuilt here
//! from the same formula and checked against `setup.metcheck` rows.
//!
//! Four `par_mod.f90` variants per precision (FLEXPART's user-edited
//! configuration file; every routine under test is verbatim): V0 as shipped;
//! V1 with `maxspec=2, maxageclass=3, nclassunc=2`; V2 as V1 with the kernel
//! off and particle-count output on; V3 as V1 with particle-count output on
//! and the kernel on (all `par_mod` parameters). V0 runs every routine; V1-V3
//! run `conccalc` (V3 only two configurations) and V1-V2 `drydepokernel`.
//!
//! * `conccalc`: 70 particles under 19 switch configurations (`ind_samp` 0 /
//!   -1 with both `memind` orders, DRY/WETBKDEP, per-release output,
//!   domain filling, nest on/off, weights 1, 0.5, 0.7, a repeated call, a
//!   north-pole grid shift, both half-cell edge strips). Every non-zero cell
//!   of `gridunc` and `griduncn` and every `creceptor` value is compared, and
//!   the port's set of non-zero cells must equal the fixture's.
//! * `drydepokernel` / `_nest`: 92 calls per variant (V0-V2) on two
//!   geometries (one making `xl` exact, for the `ddx = 0.5` tie and
//!   `-1 < xl < 0`), each on zeroed grids; every non-zero cell compared.
//!   Upstream stores deposition in `real(dep_prec)` = `real(4)` at BOTH
//!   precisions, so the port's `f64` result is rounded to `f32` for the
//!   comparison.
//! * `mean`, `centerofmass`, `clustering`: called directly at full precision.
//! * `plumetraj`: upstream's only output is a formatted record. The driver
//!   reads it back field by field, so it is compared to the precision of its
//!   own edit descriptors (half a unit of the last printed digit).
//! * `partpos_average`: the 15 accumulators after each of two calls.
//!
//! ## Results
//!
//! Taken **2026-10-02**, upstream `3d7eebf`, 12 tests, all passing.
//! "max rel dev" is over every output of every row.
//!
//! | Group | Rows | vs `real8` | vs `real4` |
//! |---|---:|---:|---:|
//! | `conccalc.grid` (mother) | 1138 | **0 (bit-exact, tol 0)** | 3.57e-5 (spread rule, 89 cells; see `CONC_GRID`) |
//! | `conccalc.gridn` (nest) | 609 | **0 (bit-exact, tol 0)** | 1.25e-6 |
//! | `conccalc.receptor` | 124 | **0 (bit-exact)** | 4.83e-6 |
//! | `drydepokernel.cell` | 226 | **0 (bit-exact after `f32` storage)** | 9.22e-7 |
//! | `drydepokernel_nest.cell` | 160 | **0 (bit-exact after `f32` storage)** | 1.13e-6 |
//! | `mean` | 6 | **0 (bit-exact)** | 9.8e-8 (+1 output by spread rule: `xs` = 0 in f32) |
//! | `centerofmass` | 5 | **0 (bit-exact)** | 1.14e-5 (spread rule, 1 output: polar ring) |
//! | `clustering` | 7 | **0 (bit-exact, tol 0)** | 7.5e-7 |
//! | `partpos_average` | 14 | **0 (bit-exact)** | 5.08e-5 (spread rule, 6 outputs: `cartx`/`carty` at 89.9 N, 179.9 E) |
//! | `plumetraj` record | 8 records, 258 values | within 0.500 of the last printed digit | within 0.500 of the last printed digit |
//!
//! Interpretation: against the real(8) build every group is bit-exact; the
//! translation, including upstream's product order, `int()` truncation and
//! in-place degree/radian round trip, is reproduced exactly on this sweep.
//! Against the shipped real(4) build, every residual above `1e-5` is
//! attributed to upstream's single precision (the doc of each bound says how).
//! `plumetraj` is verified only to the resolution of its own output format
//! (`f8.1` = 5e-5 relative at 1000 km); its components are verified at full
//! precision above. Re-print with `cargo test --release -p changi --test
//! flexpart_output_code_to_code -- --nocapture`.
//!
//! ## What this does NOT establish
//!
//! Verification, not validation, on synthetic fields. Nothing here supports
//! emergency response or dose assessment for real populations.

mod common;

use std::collections::HashMap;
use std::sync::OnceLock;

use changi::flexpart::concentration::{
    conccalc, drydepokernel, drydepokernel_nest, Attribution, ConcCalcError, ConcCalcSettings,
    ConcentrationGrid, DepositionGrid, OutputDomain, Particles, Receptors, SamplingUnit,
};
use changi::flexpart::particle_average::{partpos_average, GridGeometry, GriddedMet, ParticleAverages};
use changi::flexpart::plume_trajectory::{
    centerofmass, clustering, mean, plumetraj, PlumeParticle, PlumeRecord, ReleaseWindow, NCLUSTER,
};
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_output_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_output_real8.csv");

/// Rows carrying `real(kind=dp)` values (or printed with `rowvd`).
const DP_ROWS: &[&str] = &[
    "setup.particle",
    "setup.plume_particle",
    "plumetraj.out",
    "partpos_average",
];

/// `nxmax`, `nymax` of the shipped `par_mod.f90`: the met arrays' extent.
const NXMAX: usize = 361;
const NYMAX: usize = 181;

// ---------------------------------------------------------------------------
// Synthetic meteorology (formula shared with the driver).

#[allow(clippy::too_many_arguments)]
fn nf(
    ix: usize,
    jy: usize,
    k: usize,
    m: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    q: usize,
) -> f64 {
    ((a * ix + b * jy + c * k + d * m) % q) as f64
}

fn build_met(rows: &[Row]) -> GriddedMet {
    let r = rows
        .iter()
        .find(|r| r.function == "setup.met")
        .expect("setup.met");
    let nz = r.args[0] as usize;
    let mut met = GriddedMet::zeros(NXMAX, NYMAX, nz);
    met.height = r.args[1..=nz].to_vec();
    for m in 1..=2 {
        for k in 1..=nz {
            for jy in 0..NYMAX {
                for ix in 0..NXMAX {
                    let i3 = met.idx3(ix, jy, k - 1, m - 1);
                    let kf = k as f64;
                    met.rho[i3] =
                        1.25 - 0.0625 * kf + nf(ix, jy, k, m, 37, 101, 53, 17, 97) / 256.0;
                    met.pv[i3] = -6.0 + nf(ix, jy, k, m, 41, 13, 29, 7, 97) / 8.0;
                    met.qv[i3] = nf(ix, jy, k, m, 11, 61, 3, 23, 89) / 4096.0;
                    met.tt[i3] = 300.0 - 4.0 * kf + nf(ix, jy, k, m, 19, 7, 31, 5, 83) / 32.0;
                    met.uu[i3] = -8.0 + nf(ix, jy, k, m, 23, 17, 11, 3, 101) / 8.0;
                    met.vv[i3] = 6.0 - nf(ix, jy, k, m, 13, 29, 37, 41, 103) / 16.0;
                }
            }
        }
        for jy in 0..NYMAX {
            for ix in 0..NXMAX {
                let i2 = met.idx2s(ix, jy, m - 1);
                met.tropopause[i2] = 2000.0 + 96.0 * nf(ix, jy, 0, m, 7, 19, 0, 13, 79);
                met.hmix[i2] = 150.0 + 16.0 * nf(ix, jy, 0, m, 31, 3, 0, 11, 73);
            }
        }
    }
    for jy in 0..NYMAX {
        for ix in 0..NXMAX {
            let i2 = met.idx2(ix, jy);
            met.oro[i2] = 8.0 * nf(ix, jy, 0, 0, 29, 43, 0, 0, 89);
        }
    }
    // The formula is exact in both precisions; the echoed samples prove the
    // two sides built the same fields.
    for r in rows.iter().filter(|r| r.function == "setup.metcheck") {
        let (ix, jy, k, m) = (
            r.args[0] as usize,
            r.args[1] as usize,
            r.args[2] as usize - 1,
            r.args[3] as usize - 1,
        );
        let i3 = met.idx3(ix, jy, k, m);
        let mine = [
            met.rho[i3],
            met.pv[i3],
            met.qv[i3],
            met.tt[i3],
            met.uu[i3],
            met.vv[i3],
            met.tropopause[met.idx2s(ix, jy, m)],
            met.hmix[met.idx2s(ix, jy, m)],
            met.oro[met.idx2(ix, jy)],
        ];
        assert_eq!(
            mine.to_vec(),
            r.outs,
            "met formula mismatch at {:?}",
            r.args
        );
    }
    met
}

// ---------------------------------------------------------------------------
// Replaying the driver's sequence of calls through the port.

/// The port's result for one `conccalc` configuration.
struct ConcOut {
    grid: ConcentrationGrid,
    gridn: ConcentrationGrid,
    creceptor: Vec<f64>,
    nspec: usize,
}

/// The port's result for one deposition call: (mother, nest, kp, nunc, nage).
struct DepOut {
    mother: DepositionGrid,
    nest: DepositionGrid,
    kp: usize,
    nunc: usize,
    nage: usize,
}

struct Replay {
    conc: HashMap<i64, ConcOut>,
    dep: HashMap<i64, DepOut>,
    plume: Vec<PlumeRecord>,
}

fn replay(rows: &[Row], met: &GriddedMet) -> Replay {
    let mut conc = HashMap::new();
    let mut dep = HashMap::new();
    let mut maxspec = 1;
    let mut maxageclass = 1;
    let mut nclassunc = 1;
    let mut use_kernel = true;
    let mut count_output = false;
    let mut particles = Particles::default();
    let mut grid_row: Option<Row> = None;
    let mut receptors = Receptors::default();
    let mut plume_parts: HashMap<i64, Vec<PlumeParticle>> = HashMap::new();
    let mut plume_rel: HashMap<i64, Vec<ReleaseWindow>> = HashMap::new();
    let mut plume_time: HashMap<i64, (i64, [usize; 2], [i64; 2], i64)> = HashMap::new();
    let mut plume_geo = None;

    for r in rows {
        let a = &r.args;
        match r.function.as_str() {
            "setup.variant" => {
                maxspec = a[1] as usize;
                maxageclass = a[2] as usize;
                nclassunc = a[3] as usize;
                use_kernel = a[4] == 1.0;
                count_output = a[5] == 1.0;
                particles = Particles {
                    nspec: maxspec,
                    ..Particles::default()
                };
                receptors = Receptors::default();
            }
            "setup.particle" => {
                particles.itra1.push(a[1] as i64);
                particles.itramem.push(a[2] as i64);
                particles.xtra1.push(a[3]);
                particles.ytra1.push(a[4]);
                particles.ztra1.push(a[5]);
                particles.npoint.push(a[6] as usize - 1);
                particles.nclass.push(a[7] as usize - 1);
                particles.xmass1.push(a[8]);
                particles.xscav_frac1.push(a[9]);
                if maxspec == 2 {
                    particles.xmass1.push(a[10]);
                    particles.xscav_frac1.push(a[11]);
                }
            }
            "setup.grid" => grid_row = Some(r.clone()),
            "setup.receptor" => {
                receptors.x.push(a[1]);
                receptors.y.push(a[2]);
                receptors.area.push(a[3]);
            }
            "setup.conccalc" => {
                let g = &grid_row.as_ref().expect("setup.grid first").args;
                let (nx, ny, nzg) = (g[0] as usize, g[1] as usize, g[2] as usize);
                let nageclass = g[14] as usize;
                let settings = ConcCalcSettings {
                    sampling: if a[1] == -1.0 {
                        SamplingUnit::MassMixingRatio
                    } else {
                        SamplingUnit::Mass
                    },
                    output_for_each_release: a[4] == 1.0,
                    domain_filling: a[5] == 1.0,
                    use_kernel,
                    backward_deposition: a[3] != 0.0,
                    particle_count_output: count_output,
                    lage: g[15..15 + nageclass].iter().map(|&v| v as i64).collect(),
                    outheight: g[3..6].to_vec(),
                    dx: g[12],
                    dy: g[13],
                    nspec: maxspec,
                };
                let mother = OutputDomain {
                    numxgrid: nx,
                    numygrid: ny,
                    dxout: a[8],
                    dyout: a[9],
                    xoutshift: a[10],
                    youtshift: a[11],
                };
                let nest_dom = OutputDomain {
                    numxgrid: g[6] as usize,
                    numygrid: g[7] as usize,
                    dxout: g[8],
                    dyout: g[9],
                    xoutshift: g[10],
                    youtshift: g[11],
                };
                let mut m = met.clone();
                let m2 = a[2] as usize; // memind(2), 1-based
                m.memind = [2 - m2, m2 - 1];
                let mut grid =
                    ConcentrationGrid::zeros(nx, ny, nzg, maxspec, 3, nclassunc, maxageclass);
                let mut gridn = ConcentrationGrid::zeros(
                    nest_dom.numxgrid,
                    nest_dom.numygrid,
                    nzg,
                    maxspec,
                    3,
                    nclassunc,
                    maxageclass,
                );
                let mut creceptor = vec![0.0; receptors.x.len() * maxspec];
                let itime = 86_400;
                let calls: Vec<f64> = if a[12] == 1.0 {
                    vec![a[7], 1.0]
                } else {
                    vec![a[7]]
                };
                for w in calls {
                    let nest = if a[6] == 1.0 {
                        Some((&nest_dom, &mut gridn))
                    } else {
                        None
                    };
                    conccalc(
                        itime,
                        w,
                        &particles,
                        &settings,
                        Some(&m),
                        &mother,
                        &mut grid,
                        nest,
                        &receptors,
                        &mut creceptor,
                    )
                    .expect("sweep stays inside upstream's defined behaviour");
                }
                conc.insert(
                    a[0] as i64,
                    ConcOut {
                        grid,
                        gridn,
                        creceptor,
                        nspec: maxspec,
                    },
                );
            }
            "setup.drydepokernel" => {
                let deposit: Vec<f64> = a[3..3 + maxspec].to_vec();
                let spec: Vec<bool> = a[5..5 + maxspec].iter().map(|&v| v == 1.0).collect();
                let (nunc, nage, kp) = (a[7] as usize - 1, a[8] as usize - 1, a[9] as usize - 1);
                let (dx, dy) = (a[10], a[11]);
                let md = OutputDomain {
                    numxgrid: a[20] as usize,
                    numygrid: a[21] as usize,
                    dxout: a[12],
                    dyout: a[13],
                    xoutshift: a[14],
                    youtshift: a[15],
                };
                let nd = OutputDomain {
                    numxgrid: a[22] as usize,
                    numygrid: a[23] as usize,
                    dxout: a[16],
                    dyout: a[17],
                    xoutshift: a[18],
                    youtshift: a[19],
                };
                let mut mother = DepositionGrid::zeros(
                    md.numxgrid,
                    md.numygrid,
                    maxspec,
                    3,
                    nclassunc,
                    maxageclass,
                );
                let mut nest = DepositionGrid::zeros(
                    nd.numxgrid,
                    nd.numygrid,
                    maxspec,
                    3,
                    nclassunc,
                    maxageclass,
                );
                let att = if use_kernel {
                    Attribution::UniformKernel
                } else {
                    Attribution::DirectCell
                };
                drydepokernel(
                    &md,
                    dx,
                    dy,
                    att,
                    &deposit,
                    &spec,
                    a[1],
                    a[2],
                    nunc,
                    nage,
                    kp,
                    &mut mother,
                )
                .expect("in range");
                drydepokernel_nest(
                    &nd, dx, dy, &deposit, &spec, a[1], a[2], nunc, nage, kp, &mut nest,
                )
                .expect("in range");
                dep.insert(
                    a[0] as i64,
                    DepOut {
                        mother,
                        nest,
                        kp,
                        nunc,
                        nage,
                    },
                );
            }
            "setup.plume_particle" => {
                let ic = a[0] as i64;
                plume_parts.entry(ic).or_default().push(PlumeParticle {
                    itra1: a[2] as i64,
                    release: a[6] as usize - 1,
                    x: a[3],
                    y: a[4],
                    z: a[5],
                });
                plume_time.insert(
                    ic,
                    (
                        a[7] as i64,
                        [a[8] as usize - 1, a[9] as usize - 1],
                        [a[10] as i64, a[11] as i64],
                        a[12] as i64,
                    ),
                );
            }
            "setup.release" => {
                plume_rel
                    .entry(a[0] as i64)
                    .or_default()
                    .push(ReleaseWindow {
                        start: a[2] as i64,
                        end: a[3] as i64,
                    });
            }
            "setup.plume_grid" => {
                plume_geo = Some(GridGeometry {
                    xlon0: a[0],
                    ylat0: a[1],
                    dx: a[2],
                    dy: a[3],
                });
            }
            _ => {}
        }
    }

    let mut plume = Vec::new();
    if let Some(geo) = plume_geo {
        let mut ics: Vec<i64> = plume_parts.keys().copied().collect();
        ics.sort_unstable();
        for ic in ics {
            let (itime, memind, memtime, lage_last) = plume_time[&ic];
            let mut m = met.clone();
            m.memind = memind;
            m.memtime = memtime;
            plume.extend(
                plumetraj(
                    itime,
                    &plume_parts[&ic],
                    &plume_rel[&ic],
                    lage_last,
                    &m,
                    &geo,
                )
                .expect("plumetraj"),
            );
        }
    }
    Replay { conc, dep, plume }
}

struct Setup {
    fx: Fixtures,
    met: GriddedMet,
    r4: Replay,
    r8: Replay,
}

fn setup() -> &'static Setup {
    static S: OnceLock<Setup> = OnceLock::new();
    S.get_or_init(|| {
        let fx = Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, DP_ROWS);
        let met = build_met(&fx.real8);
        let met4 = build_met(&fx.real4);
        assert_eq!(
            met, met4,
            "the synthetic met must be identical at both precisions"
        );
        let r4 = replay(&fx.real4, &met);
        let r8 = replay(&fx.real8, &met);
        Setup { fx, met, r4, r8 }
    })
}

fn rep(p: Precision) -> &'static Replay {
    let s = setup();
    if p == Precision::Real4 {
        &s.r4
    } else {
        &s.r8
    }
}

fn all(_: &Row, _: Precision) -> bool {
    true
}

// ---------------------------------------------------------------------------
// conccalc

fn eval_conc(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let a = &r.args;
    let out = rep(p).conc.get(&(a[0] as i64))?;
    let idx = |g: &ConcentrationGrid| {
        g.index(
            a[1] as usize,
            a[2] as usize,
            a[3] as usize - 1,
            a[4] as usize - 1,
            a[5] as usize - 1,
            a[6] as usize - 1,
            a[7] as usize - 1,
        )
    };
    Some(match r.function.as_str() {
        "conccalc.grid" => vec![out.grid.values[idx(&out.grid)]],
        "conccalc.gridn" => vec![out.gridn.values[idx(&out.gridn)]],
        "conccalc.receptor" => {
            vec![out.creceptor[(a[1] as usize - 1) * out.nspec + a[2] as usize - 1]]
        }
        _ => return None,
    })
}

/// Real(4): upstream stores the output-cell coordinate `xl`, `yl` in
/// default `real`. For `xl` in `[4, 8)` that is an absolute error up to
/// `4.8e-7`, which the kernel weights `1.5 - ddx`, `1 - wx` (down to ~0.02)
/// amplify to `~3e-5` relative. Attributed by experiment, 2026-10-02: with
/// the port's `xl`, `yl` rounded to `f32` (a scratch edit, reverted), the
/// worst real(4) deviation on the mother grid fell from `3.57e-5` (89 of
/// 1138 cells above `1e-5`) to `2.3e-7` (none); the nest's from `1.25e-6`
/// to `2.0e-7`. Hence the spread rule on the one
/// output column. Real(8) is held **bit-exact** (`tol8 = 0`): the
/// mother/nest product-order difference is a last-bit effect, and only a zero
/// tolerance can see it.
const CONC_GRID: Bounds = Bounds {
    tol8: 0.0,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(&[0]),
};

#[test]
fn conccalc_output_grids_match_upstream() {
    check_group(&setup().fx, "conccalc.grid", CONC_GRID, all, eval_conc);
    check_group(&setup().fx, "conccalc.gridn", CONC_GRID, all, eval_conc);
}

#[test]
fn conccalc_receptors_match_upstream() {
    let b = Bounds::rel(1e-13, 1e-5);
    check_group(&setup().fx, "conccalc.receptor", b, all, eval_conc);
}

/// The value checks only visit cells the Fortran filled; this checks the port
/// fills no others (and that the sweep reached every branch it claims to).
#[test]
fn conccalc_fills_exactly_upstreams_cells() {
    let s = setup();
    for p in [Precision::Real4, Precision::Real8] {
        let rows = s.fx.rows(p);
        for (cid, out) in &rep(p).conc {
            let nz = |g: &ConcentrationGrid| g.values.iter().filter(|v| **v != 0.0).count();
            let want = |name: &str| {
                rows.iter()
                    .filter(|r| r.function == name && r.args[0] as i64 == *cid)
                    .count()
            };
            assert_eq!(
                nz(&out.grid),
                want("conccalc.grid"),
                "{} cfg {cid}: mother non-zero cells",
                p.label()
            );
            assert_eq!(
                nz(&out.gridn),
                want("conccalc.gridn"),
                "{} cfg {cid}: nest non-zero cells",
                p.label()
            );
        }
        let receptor_hits = rows
            .iter()
            .filter(|r| r.function == "conccalc.receptor" && r.outs[0] != 0.0)
            .count();
        assert!(
            receptor_hits >= 20,
            "{}: receptor kernel exercised ({receptor_hits} non-zero)",
            p.label()
        );
    }
}

/// Upstream defect, observed: with `lparticlecountoutput = .true.` and the
/// kernel on (variant V3), direct attribution adds particle COUNTS but the
/// kernel branch still adds `xmass/rhoi*weight*w` (it never tests the
/// switch), so one grid mixes counts and masses. The fixture shows both
/// kinds of cell, and the port reproduces every one of them (checked above).
#[test]
fn particle_count_mode_mixes_counts_and_masses_upstream() {
    for p in [Precision::Real4, Precision::Real8] {
        let cells: Vec<f64> = setup()
            .fx
            .rows(p)
            .iter()
            .filter(|r| r.function == "conccalc.grid" && r.args[0] == 301.0)
            .map(|r| r.outs[0])
            .collect();
        let counts = cells.iter().filter(|v| v.fract() == 0.0).count();
        let masses = cells.len() - counts;
        println!("{} V3 cfg 301: {counts} integer (count) cells, {masses} fractional (kernel mass) cells", p.label());
        assert!(counts > 0 && masses > 0);
    }
}

/// Refusals where upstream's behaviour is undefined.
#[test]
fn conccalc_refuses_undefined_upstream_states() {
    let s = setup();
    let base = Particles {
        itra1: vec![0],
        itramem: vec![-50_000],
        xtra1: vec![3.3],
        ytra1: vec![2.2],
        ztra1: vec![10.0],
        npoint: vec![0],
        nclass: vec![0],
        nspec: 1,
        xmass1: vec![1.0],
        xscav_frac1: vec![0.0],
    };
    let set = ConcCalcSettings {
        sampling: SamplingUnit::MassMixingRatio,
        output_for_each_release: false,
        domain_filling: false,
        use_kernel: true,
        backward_deposition: false,
        particle_count_output: false,
        lage: vec![40_000],
        outheight: vec![100.0],
        dx: 0.5,
        dy: 0.5,
        nspec: 1,
    };
    let dom = OutputDomain {
        numxgrid: 4,
        numygrid: 4,
        dxout: 1.0,
        dyout: 1.0,
        xoutshift: 0.0,
        youtshift: 0.0,
    };
    let mut g = ConcentrationGrid::zeros(4, 4, 1, 1, 1, 1, 1);
    let rec = Receptors::default();
    let e = conccalc(
        0,
        1.0,
        &base,
        &set,
        Some(&s.met),
        &dom,
        &mut g,
        None,
        &rec,
        &mut [],
    );
    assert_eq!(e, Err(ConcCalcError::AgeBeyondLastClass(0)));
    let mut high = base.clone();
    high.itramem = vec![0];
    high.ztra1 = vec![8000.0];
    let e = conccalc(
        0,
        1.0,
        &high,
        &set,
        Some(&s.met),
        &dom,
        &mut g,
        None,
        &rec,
        &mut [],
    );
    assert_eq!(e, Err(ConcCalcError::AboveTopLevel(0)));
    assert!(
        g.values.iter().all(|v| *v == 0.0),
        "a refusal writes nothing"
    );
}

// ---------------------------------------------------------------------------
// drydepokernel

fn eval_dep(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let a = &r.args;
    let d = rep(p).dep.get(&(a[0] as i64))?;
    let g = match r.function.as_str() {
        "drydepokernel.cell" => &d.mother,
        "drydepokernel_nest.cell" => &d.nest,
        _ => return None,
    };
    let v = g.values[g.index(
        a[1] as usize,
        a[2] as usize,
        a[3] as usize - 1,
        d.kp,
        d.nunc,
        d.nage,
    )];
    // Upstream's grid is real(dep_prec) = real(4) in both builds.
    Some(vec![v as f32 as f64])
}

#[test]
fn drydepokernel_matches_upstream() {
    let b = Bounds::rel(0.0, 1e-5);
    check_group(&setup().fx, "drydepokernel.cell", b, all, eval_dep);
    check_group(&setup().fx, "drydepokernel_nest.cell", b, all, eval_dep);
    let s = setup();
    for p in [Precision::Real4, Precision::Real8] {
        let rows = s.fx.rows(p);
        for (cid, d) in &rep(p).dep {
            let nz = |g: &DepositionGrid| g.values.iter().filter(|v| (**v as f32) != 0.0).count();
            let want = |name: &str| {
                rows.iter()
                    .filter(|r| r.function == name && r.args[0] as i64 == *cid)
                    .count()
            };
            assert_eq!(
                nz(&d.mother),
                want("drydepokernel.cell"),
                "{} call {cid}: mother cells",
                p.label()
            );
            assert_eq!(
                nz(&d.nest),
                want("drydepokernel_nest.cell"),
                "{} call {cid}: nest cells",
                p.label()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// mean, centerofmass, clustering

fn eval_stats(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let a = &r.args;
    let n = a[0] as usize;
    Some(match r.function.as_str() {
        "mean" => {
            let (xm, xs) = mean(&a[1..=n]);
            vec![xm, xs]
        }
        "centerofmass" => {
            let (xc, yc) = centerofmass(&a[1..=n], &a[1 + n..=2 * n]);
            vec![xc, yc]
        }
        "clustering" => {
            let mut xl = a[1..=n].to_vec();
            let mut yl = a[1 + n..=2 * n].to_vec();
            let zl = &a[1 + 2 * n..=3 * n];
            let res = clustering(&mut xl, &mut yl, zl);
            let mut o = xl;
            o.extend(yl);
            match res {
                Some(c) => {
                    o.extend(c.xclust);
                    o.extend(c.yclust);
                    o.extend(c.zclust);
                    o.extend(c.fclust);
                    o.push(c.rms);
                    o.extend(c.rmsclust);
                    o.push(c.zrms);
                }
                // Upstream returned without touching its outputs, which the
                // driver had preset to -999.
                None => o.extend(std::iter::repeat_n(p.lit(-999.0), 5 * NCLUSTER + 2)),
            }
            o
        }
        _ => return None,
    })
}

/// Real(4): the one-pass variance `xq - xl*xl/n` cancels catastrophically
/// for `5000.01 .. 5000.09` (true sigma 0.0274, terms ~2.25e8): the shipped
/// build gets `xaux < 1e-30` and returns `xs = 0`, its real(8) build 0.0274.
/// Spread rule on `xs` only.
const MEAN: Bounds = Bounds {
    tol8: 1e-13,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(&[1]),
};

#[test]
fn mean_matches_upstream() {
    check_group(&setup().fx, "mean", MEAN, all, eval_stats);
}

/// Real(4): for the ring of points at 88.7-89.6 N the mean unit vector's
/// horizontal components are sums of `cos(lat) sin(lon)` terms that cancel
/// to `~1e-3` of their size, so the centre's longitude is ill-conditioned
/// (shipped build `1.1e-5` relative from its own real(8) value). Spread rule
/// on the longitude only.
const CENTRE: Bounds = Bounds {
    tol8: 1e-13,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(&[0]),
};

#[test]
fn centerofmass_matches_upstream() {
    check_group(&setup().fx, "centerofmass", CENTRE, all, eval_stats);
}

/// Real(8) is held **bit-exact** (`tol8 = 0`): upstream's in-place
/// `x*pi180/pi180` round trip changes four of set 7's inputs by one ulp
/// (15.14, 30.92, 31.33, 31.74), and only a zero tolerance can see whether
/// the port reproduces it.
const CLUSTER: Bounds = Bounds {
    tol8: 0.0,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::Strict,
};

#[test]
fn clustering_matches_upstream() {
    check_group(&setup().fx, "clustering", CLUSTER, all, eval_stats);
}

// ---------------------------------------------------------------------------
// partpos_average

fn eval_partpos(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let a = &r.args;
    let s = setup();
    let geo = GridGeometry {
        xlon0: a[6],
        ylat0: a[7],
        dx: 1.0,
        dy: a[8],
    };
    let mut acc = ParticleAverages::default();
    // Replay the calls up to and including this one (the accumulators carry).
    for ic in 1..=(a[1] as usize) {
        let mut m = s.met.clone();
        m.memtime = [0, 10_800];
        let (itime, memind) = if ic == 1 {
            (3600, [0, 1])
        } else {
            (9000, [1, 0])
        };
        m.memind = memind;
        partpos_average(&mut acc, itime, a[3], a[4], a[5], &m, &geo)?;
    }
    let _ = p;
    Some(vec![
        acc.count as f64,
        acc.cartx,
        acc.carty,
        acc.cartz,
        acc.z,
        acc.topo,
        acc.pv,
        acc.qv,
        acc.tt,
        acc.uu,
        acc.vv,
        acc.rho,
        acc.tro,
        acc.hmix,
        acc.energy,
    ])
}

/// Real(4): `x = cos(ylat) sin(xlon)`, `y = -cos(ylat) cos(xlon)` lose
/// relative precision where a factor is near zero: at 89.93 N (`cos ~ 1.2e-3`)
/// and at 179.9 E (`sin ~ 1.7e-3`) the `f32` angle's rounding (`~1e-7` rad)
/// becomes `~5e-5` relative. Spread rule on `cartx`, `carty` only; every
/// other accumulator is held at `1e-5`.
const PARTPOS: Bounds = Bounds {
    tol8: 1e-13,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(&[1, 2]),
};

#[test]
fn partpos_average_matches_upstream() {
    check_group(&setup().fx, "partpos_average", PARTPOS, all, eval_partpos);
}

// ---------------------------------------------------------------------------
// plumetraj: compared to the precision of upstream's formatted record.

/// Decimals of each value in the record after `i5,i8`:
/// `2f9.4,4f8.1,f8.2,4f8.1,3f6.1,5(2f8.3,f7.0,f6.1,f8.1)`.
fn plume_decimals() -> Vec<i32> {
    let mut d = vec![4, 4, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1];
    for _ in 0..NCLUSTER {
        d.extend([3, 3, 0, 1, 1]);
    }
    d
}

fn plume_values(rec: &PlumeRecord) -> Vec<Option<f64>> {
    let c = rec.clusters;
    let mut v = vec![
        Some(rec.xcenter),
        Some(rec.ycenter),
        Some(rec.zcenter),
        Some(rec.topocenter),
        Some(rec.hmixcenter),
        Some(rec.tropocenter),
        Some(rec.pvcenter),
        Some(rec.rmsdist),
        c.map(|c| c.rms),
        Some(rec.zrmsdist),
        c.map(|c| c.zrms),
        Some(rec.hmixfract),
        Some(rec.pvfract),
        Some(rec.tropofract),
    ];
    for k in 0..NCLUSTER {
        v.push(c.map(|c| c.xclust[k]));
        v.push(c.map(|c| c.yclust[k]));
        v.push(c.map(|c| c.zclust[k]));
        v.push(c.map(|c| c.fclust[k]));
        v.push(c.map(|c| c.rmsclust[k]));
    }
    v
}

#[test]
fn plumetraj_matches_upstreams_record() {
    let s = setup();
    let dec = plume_decimals();
    for p in [Precision::Real8, Precision::Real4] {
        let rows: Vec<&Row> =
            s.fx.rows(p)
                .iter()
                .filter(|r| r.function == "plumetraj.out")
                .collect();
        let recs = &rep(p).plume;
        assert_eq!(
            rows.len(),
            recs.len(),
            "{}: one record per release point with particles",
            p.label()
        );
        let mut worst_units = 0.0_f64;
        let mut compared = 0;
        let mut stale_blocks = 0;
        for (k, (r, rec)) in rows.iter().zip(recs).enumerate() {
            assert_eq!(
                r.args[1] as usize,
                rec.release + 1,
                "record {k}: release point"
            );
            assert_eq!(
                r.args[2] as i64, rec.time_offset,
                "record {k}: integer-division time offset"
            );
            for (col, (want, got)) in r.outs.iter().zip(plume_values(rec)).enumerate() {
                let Some(got) = got else {
                    continue; // upstream printed values it never assigned
                };
                let unit = 10f64.powi(-dec[col]);
                let diff = (got - want).abs();
                // real(8): the port must print as upstream printed (half a
                // unit of the last digit). real(4): plus the shipped build's
                // own rounding, 1e-5 of the value.
                let bound = 0.5 * unit * (1.0 + 1e-9)
                    + if p == Precision::Real4 {
                        1e-5 * want.abs()
                    } else {
                        0.0
                    };
                assert!(
                    diff <= bound,
                    "{} record {k} col {col}: port {got} vs printed {want} (bound {bound:e})",
                    p.label()
                );
                worst_units = worst_units.max(diff / unit);
                compared += 1;
            }
            if rec.clusters.is_none() {
                // The quirk, observed: the unassigned cluster block repeats
                // the previous record's (stack locals left by release 1).
                let prev = rows[k - 1];
                assert_eq!(
                    r.outs[14..],
                    prev.outs[14..],
                    "stale cluster block in record {k}"
                );
                stale_blocks += 1;
            }
        }
        println!(
            "{} plumetraj.out  {} records, {compared} values, max |port - printed| = {worst_units:.3} units of the last printed digit, {stale_blocks} stale cluster blocks",
            p.label(),
            rows.len()
        );
        assert!(stale_blocks > 0, "the n < ncluster case is in the sweep");
    }
}

#[test]
fn plumetraj_skips_old_and_empty_releases() {
    // Release 3 (|start - itime| > lage) and release 5 (no particle) write no
    // record; releases 1, 2, 4 and 6 (|start - itime| == lage exactly) do,
    // in both calls.
    let got: Vec<usize> = rep(Precision::Real8)
        .plume
        .iter()
        .map(|r| r.release + 1)
        .collect();
    assert_eq!(got, vec![1, 2, 4, 6, 1, 2, 4, 6]);
}
