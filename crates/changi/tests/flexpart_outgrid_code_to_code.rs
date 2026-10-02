// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::{outgrid, fluxes,
// initial_condition}` against upstream FLEXPART v10.4 (commit 3d7eebf,
// GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage "outgrid" — output-grid set-up, fluxes, initial conditions
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_outgrid.f90` links upstream's `outgrid_init`,
//! `outgrid_init_nest`, `calcfluxes`, `fluxoutput`, `initial_cond_calc` and
//! `caldate` verbatim (with `com_mod`, `unc_mod`, `outg_mod`, `flux_mod`,
//! `oh_mod`), built by `dev/build_reference_outgrid.sh` against a `par_mod.f90`
//! configuration copy with exactly three parameter lines changed
//! (`maxnests=2, nxmaxn=12, nymaxn=12`; `maxspec=2`; `maxageclass=2`). Every
//! synthetic input is dyadic or an `f32`-exact literal, written straight into
//! the modules; topography, nest topography and density are formulas of
//! their indices, rebuilt here and checked against echoed samples.
//!
//! * `outgrid_init` / `outgrid_init_nest`: nine grid placements (`outgrid.config`)
//!   covering row boundaries exactly at 0°, rows straddling the equator, both
//!   hemispheres, polar rows, 0.25° rows at the equator, topography samples in
//!   the mother grid, in two nests and their overlap, and within
//!   `eps = nxmax/3e5` of a nest edge. Compared: area, volumes, east and north
//!   wall areas of the first and last column of every row, and `oroout` /
//!   `orooutn` of every cell.
//! * `calcfluxes`: 174 particle steps (14 hand-picked edge cases, 160
//!   generated dyadic steps), each on a zeroed `flux` (every non-zero cell and
//!   the non-zero count compared, so the port's set of cells must equal
//!   upstream's), then all of them accumulated.
//! * `fluxoutput`: called twice (on the accumulated field and on a synthetic
//!   field with dense, sparse, borderline-sparse and negative-valued blocks).
//!   Upstream WRITES A FILE; the driver reads that file back record by record
//!   (no fluxoutput arithmetic is copied into the driver). Compared: file
//!   name date and time, the time record, each block's layout and length, every
//!   sparse `(index, value)` and dense value, the block order, and the reset.
//!   The port is given upstream's own wall areas (`fixture_geometry`), so this
//!   group measures `fluxoutput` alone.
//! * `initial_cond_calc`: 134 particles (14 hand-picked: the `ddx = 0.5` tie,
//!   `ddx = 0.5 -+ 1/64`, all four edge strips, `xl = -0.5` and `-1`, above the
//!   output grid, not at `itime`; 120 generated), both `linit_cond` units, both
//!   `memind` orders and the release-slot switches; each on a zeroed
//!   `init_cond`, then accumulated.
//!
//! ## Results
//!
//! Taken **2026-10-02**, upstream `3d7eebf`, 4 896 rows per fixture, 13 tests,
//! all passing. The real(8) bound is **0** (bit-exact required) for every group.
//!
//! | Group | Rows | vs `real8` | vs `real4` |
//! |---|---:|---:|---:|
//! | `outgrid_init.cell` | 238 (3 094 outputs) | **0 (bit-exact)** | wall areas < 1e-5; area and volumes up to 9.19e-4, 270 outputs by the spread rule (see below) |
//! | `outgrid_init_nest.cell` | 238 (1 190) | **0 (bit-exact)** | as above, 270 outputs by the spread rule |
//! | `outgrid_init.oro` | 425 | **0 (bit-exact)** | 2.67e-6 |
//! | `outgrid_init_nest.oro` | 425 | **0 (bit-exact)** | 2.67e-6 |
//! | `calcfluxes.call` (non-zero count) | 174 | **0 (exact)** | **0 (exact)** |
//! | `calcfluxes.cell` | 416 | **0 (bit-exact)** | **0 (bit-exact)** |
//! | `calcfluxes.total` (accumulated) | 386 | **0 (bit-exact)** | **0 (bit-exact)** |
//! | `fluxoutput.file`, `.itime`, `.block`, `.reset` | 2, 2, 96, 2 | **exact** | **exact** |
//! | `fluxoutput.sparse` | 650 | **0 (bit-exact)** | 1.10e-7 |
//! | `fluxoutput.dense` | 1 152 (6 912 values) | **0 (bit-exact)** | 1.10e-7 |
//! | `initial_cond_calc.call` (non-zero count) | 134 | **0 (exact)** | **0 (exact)** |
//! | `initial_cond_calc.cell` | 342 | **0 (bit-exact)** | 1.34e-7 |
//! | `initial_cond_calc.total` (accumulated) | 184 | **0 (bit-exact)** | 1.34e-7 |
//!
//! **Interpretation.** Against real(8) every output of every group is
//! bit-exact: the translation, including upstream's operation order,
//! `int()` truncation, the nest-selection margin and the file layout, is
//! reproduced exactly on this sweep. `calcfluxes` is bit-exact against real(4)
//! too, because it only adds dyadic masses.
//!
//! **The shipped build's cell areas are ill-conditioned.** Upstream's zone
//! height is a difference of two `sqrt(1 - cos^2)` values. Near the equator
//! `1 - cos^2` cancels; near the poles, and for any narrow row, the two
//! square roots nearly cancel. The condition number `K` of the formula is 92
//! for a 2.5° row at 45° and reaches 7.9e4 for 0.25° rows at the equator, so
//! the real(4) build's areas and volumes are off by 1e-6 to 9.2e-4 relative
//! (polar 0.5° rows of the flux grid: up to 8.3e-5). That attribution is
//! tested on its own, not assumed:
//! `outgrid_area_real4_residual_is_the_formulas_conditioning` derives the
//! bound `u (K + 8)` (`u = 2^-24`) from the row geometry alone and every one
//! of the 476 real(4) area rows lies inside it (at most 0.648 of it). The
//! spread rule is therefore applied to the area and volume columns only; the
//! wall areas, which do not cancel, are held strictly. Any `fluxoutput`
//! density divided by `area` inherits the same error in the shipped model.
//!
//! **Upstream findings this stage pins.** `calcfluxes` drops every
//! cyclic-boundary crossing (`real(nxmin1)-1.e5`, almost certainly meant as
//! `1.e-5`): the sweep's wrapping steps record no zonal flux upstream, and a
//! mutation "fixing" it is killed (`calcfluxes_wrapping_steps_record_no_zonal_flux`).
//! `outgrid_init` zeroes only `flux(1:5,...)`: a driver diagnostic (a comment
//! line in the fixture) found all 1 536 `flux(6,...)` values still holding an
//! allocator sentinel after the call.
//!
//! ## Non-vacuity
//!
//! 35 mutations of the port (2026-10-02): 34 killed, one equivalent (moving
//! `initial_cond_calc`'s `ddx = 0.5` tie to the other branch; the neighbour
//! weight is 0 on both sides, so the tie is invisible). Three first-pass
//! survivors were sweep gaps and were closed: no block between the 4x and
//! 5x sparse thresholds (the synthetic field gained one), the port's block
//! order was not compared, and no particle had `ddx` within 1/64 of 0.5.
//!
//! ## What this does NOT establish
//!
//! Verification, not validation, on synthetic inputs. The file writing of
//! `fluxoutput` is not ported. Nothing here supports emergency response or
//! dose assessment for real populations.

mod common;

use std::collections::HashMap;
use std::sync::OnceLock;

use changi::flexpart::concentration::{ConcentrationGrid, OutputDomain};
use changi::flexpart::fluxes::{
    calcfluxes, fluxoutput, FluxGrid, FluxLayout, FluxOutput, FluxSettings, ParticleStep, DOWNWARD,
    EAST_TO_WEST, NORTH_TO_SOUTH, SOUTH_TO_NORTH, UPWARD, WEST_TO_EAST,
};
use changi::flexpart::initial_condition::{
    initial_cond_calc, InitCondError, InitCondParticle, InitCondSettings, InitCondUnit,
};
use changi::flexpart::met_fields::Field2;
use changi::flexpart::outgrid::{
    cell_geometry, output_orography, CellGeometry, NestOrography, OutputGrid,
};
use changi::flexpart::particle_average::{GridGeometry, GriddedMet};
use changi::flexpart::wet_deposition::NestFrame;
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_outgrid_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_outgrid_real8.csv");

/// Rows printed with `rowvd` (17 digits at both precisions).
const DP_ROWS: &[&str] = &["calcfluxes.call", "initial_cond_calc.call", "setup.bdate"];

/// `nxmax`, `nymax` of the shipped `par_mod.f90`.
const NXMAX: usize = 361;
const NYMAX: usize = 181;

fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, DP_ROWS))
}

fn rows(p: Precision) -> &'static [Row] {
    fixtures().rows(p)
}

fn find<'a>(rs: &'a [Row], name: &str) -> &'a Row {
    rs.iter()
        .find(|r| r.function == name)
        .unwrap_or_else(|| panic!("no `{name}` row"))
}

// ---------------------------------------------------------------------------
// Topography and nests (formula shared with the driver's setup_orography).

struct OroSetup {
    met: GridGeometry,
    eps: f64,
    oro: Field2,
    nests: Vec<NestOrography>,
}

fn oro_setup(p: Precision) -> OroSetup {
    let rs = rows(p);
    let m = find(rs, "setup.met");
    let met = GridGeometry {
        xlon0: m.args[0],
        ylat0: m.args[1],
        dx: m.args[2],
        dy: m.args[3],
    };
    let eps = m.args[4];
    assert_eq!(m.args[5] as usize, NXMAX);
    assert_eq!(m.args[6] as usize, NYMAX);
    let (nxn, nyn) = (m.args[7] as usize, m.args[8] as usize);
    let mut oro = Field2::filled(NXMAX, NYMAX, 0.0);
    for jy in 0..NYMAX {
        for ix in 0..NXMAX {
            oro.set(ix, jy, 8.0 * ((29 * ix + 43 * jy) % 89) as f64);
        }
    }
    let mut nests = Vec::new();
    for r in rs.iter().filter(|r| r.function == "setup.nest") {
        let n = r.args[0] as usize;
        let mut oron = Field2::filled(nxn, nyn, 0.0);
        for jy in 0..nyn {
            for ix in 0..nxn {
                oron.set(ix, jy, 4.0 * ((13 * ix + 7 * jy + 5 * n) % 61) as f64 + 3.0);
            }
        }
        nests.push(NestOrography {
            frame: NestFrame {
                xl: r.args[1],
                yl: r.args[2],
                xr: r.args[3],
                yr: r.args[4],
                xresol: r.args[5],
                yresol: r.args[6],
            },
            oron,
        });
    }
    assert_eq!(nests.len(), m.args[9] as usize);
    for r in rs.iter().filter(|r| r.function == "setup.orocheck") {
        let (g, ix, jy) = (r.args[0] as usize, r.args[1] as usize, r.args[2] as usize);
        let mine = if g == 0 {
            oro.at(ix, jy)
        } else {
            nests[g - 1].oron.at(ix, jy)
        };
        assert_eq!(
            mine, r.outs[0],
            "orography formula mismatch at {:?}",
            r.args
        );
    }
    OroSetup {
        met,
        eps,
        oro,
        nests,
    }
}

fn oro_setups() -> &'static [OroSetup; 2] {
    static S: OnceLock<[OroSetup; 2]> = OnceLock::new();
    S.get_or_init(|| [oro_setup(Precision::Real4), oro_setup(Precision::Real8)])
}

fn oro_for(p: Precision) -> &'static OroSetup {
    &oro_setups()[match p {
        Precision::Real4 => 0,
        Precision::Real8 => 1,
    }]
}

/// The grid and level tops of output configuration `cfg`.
fn config(p: Precision, cfg: usize) -> (OutputGrid, Vec<f64>) {
    let r = rows(p)
        .iter()
        .find(|r| r.function == "outgrid.config" && r.args[0] as usize == cfg)
        .unwrap_or_else(|| panic!("no config {cfg}"));
    let nz = r.args[7] as usize;
    (
        OutputGrid {
            outlon0: r.args[1],
            outlat0: r.args[2],
            dxout: r.args[3],
            dyout: r.args[4],
            numxgrid: r.args[5] as usize,
            numygrid: r.args[6] as usize,
        },
        r.args[8..8 + nz].to_vec(),
    )
}

fn geometry(p: Precision, cfg: usize) -> CellGeometry {
    let (g, h) = config(p, cfg);
    cell_geometry(&g, &h)
}

fn orography(p: Precision, cfg: usize) -> Vec<f64> {
    let (g, _) = config(p, cfg);
    let s = oro_for(p);
    output_orography(&g, &s.met, &s.oro, &s.nests, s.eps).expect("sample inside the arrays")
}

// ---------------------------------------------------------------------------
// outgrid_init / outgrid_init_nest.

/// `outgrid_init.cell` outputs: area, volume(1:4), areaeast(1:4),
/// areanorth(1:4) at `(ix, jy)`.
fn eval_cell(r: &Row, p: Precision, nest: bool) -> Option<Vec<f64>> {
    let (cfg, ix, jy) = (r.args[0] as usize, r.args[1] as usize, r.args[2] as usize);
    let g = geometry(p, cfg);
    let mut out = vec![g.area[g.idx2(ix, jy)]];
    out.extend((0..g.numzgrid).map(|kz| g.volume[g.idx3(ix, jy, kz)]));
    if !nest {
        out.extend((0..g.numzgrid).map(|kz| g.areaeast[g.idx3(ix, jy, kz)]));
        out.extend((0..g.numzgrid).map(|kz| g.areanorth[g.idx3(ix, jy, kz)]));
    }
    Some(out)
}

fn eval_oro(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let (cfg, ix, jy) = (r.args[0] as usize, r.args[1] as usize, r.args[2] as usize);
    let (g, _) = config(p, cfg);
    Some(vec![orography(p, cfg)[jy * g.numxgrid + ix]])
}

/// Cell geometry. Real(8): bit-exact. Real(4): the horizontal area (and the
/// volumes, `area * dh`) come from `sqrt(1-cos^2)` differences that are
/// ill-conditioned in `f32` everywhere (see
/// [`outgrid_area_real4_residual_is_the_formulas_conditioning`]), so columns 0
/// (area) and 1-4 (volumes) use the precision-spread rule; the wall areas
/// (columns 5-12) are well conditioned and held strictly.
const CELL: Bounds = Bounds {
    tol8: 0.0,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpreadOn(&[0, 1, 2, 3, 4]),
};
/// The nest rows carry only area and volumes (columns 0-4).
const CELL_NEST: Bounds = CELL;
const ORO: Bounds = Bounds::rel(0.0, 1e-5);

#[test]
fn outgrid_init_cell_geometry() {
    check_group(
        fixtures(),
        "outgrid_init.cell",
        CELL,
        |_, _| true,
        |r, p| eval_cell(r, p, false),
    );
}

#[test]
fn outgrid_init_nest_cell_geometry() {
    check_group(
        fixtures(),
        "outgrid_init_nest.cell",
        CELL_NEST,
        |_, _| true,
        |r, p| eval_cell(r, p, true),
    );
}

/// The attribution of the real(4) area residual, made falsifiable: upstream's
/// zone height is `|sqrt(1-cp^2) - sqrt(1-cm^2)|` with `cp`, `cm` the cosines
/// of the row's edges. A relative rounding `u` (`f32`: `2^-24`) in each
/// operation propagates to a relative error of at most about
/// `u * (K + 8)` in the area, with the condition number
/// `K = (cp^2/sp + cm^2/sm + sp + sm) / |sp - sm|` (`s = sqrt(1-c^2)`; a term
/// with `s = 0`, exact in both precisions, is dropped) and 8 roundings in the
/// remaining products. `K` is computed here from the row geometry alone, not
/// from the comparison. Every real(4) area must lie inside that bound
/// (measured: at most 0.65 of it, `K` from 92 to 7.9e4).
#[test]
fn outgrid_area_real4_residual_is_the_formulas_conditioning() {
    let u = f64::from(f32::EPSILON) / 2.0;
    let pi180 = changi::flexpart::constants::PI180;
    let mut worst = 0.0_f64;
    let mut n = 0;
    for r in rows(Precision::Real4)
        .iter()
        .filter(|r| r.function == "outgrid_init.cell" || r.function == "outgrid_init_nest.cell")
    {
        let (cfg, ix, jy) = (r.args[0] as usize, r.args[1] as usize, r.args[2] as usize);
        let (g, _) = config(Precision::Real4, cfg);
        let geo = geometry(Precision::Real4, cfg);
        let a = geo.area[geo.idx2(ix, jy)];
        let ylat = g.outlat0 + (jy as f64 + 0.5) * g.dyout;
        let (lp, lm) = (ylat + 0.5 * g.dyout, ylat - 0.5 * g.dyout);
        let k = if lm < 0.0 && lp > 0.0 {
            0.0
        } else {
            let (cp, cm) = ((lp * pi180).cos(), (lm * pi180).cos());
            let (sp, sm) = ((1.0 - cp * cp).sqrt(), (1.0 - cm * cm).sqrt());
            let term = |c: f64, s: f64| if s == 0.0 { 0.0 } else { c * c / s };
            (term(cp, sp) + term(cm, sm) + sp + sm) / (sp - sm).abs()
        };
        let rel = (r.outs[0] - a).abs() / a;
        let ratio = rel / (u * (k + 8.0));
        assert!(
            ratio <= 1.0,
            "cfg {cfg} row {jy}: real(4) area off by {rel:e}, beyond the conditioning bound (K = {k:e})"
        );
        worst = worst.max(ratio);
        n += 1;
    }
    println!("real4 area residual / conditioning bound: max {worst:.3} over {n} rows");
}

#[test]
fn outgrid_init_orography() {
    check_group(fixtures(), "outgrid_init.oro", ORO, |_, _| true, eval_oro);
}

#[test]
fn outgrid_init_nest_orography() {
    check_group(
        fixtures(),
        "outgrid_init_nest.oro",
        ORO,
        |_, _| true,
        eval_oro,
    );
}

// ---------------------------------------------------------------------------
// calcfluxes.

fn flux_setup(p: Precision) -> (FluxSettings, OutputDomain) {
    let r = find(rows(p), "setup.flux");
    let a = &r.args;
    (
        FluxSettings {
            output_for_each_release: false,
            domain_filling: false,
            dx: a[0],
            dy: a[1],
            nx: a[2] as i64,
            outheight: a[10..14].to_vec(),
            outheighthalf: a[14..18].to_vec(),
        },
        OutputDomain {
            numxgrid: a[8] as usize,
            numygrid: a[9] as usize,
            dxout: a[6],
            dyout: a[7],
            xoutshift: a[4],
            youtshift: a[5],
        },
    )
}

fn empty_flux(dom: &OutputDomain, nz: usize) -> FluxGrid {
    FluxGrid::zeros(dom.numxgrid, dom.numygrid, nz, 2, 2, 2)
}

/// Apply the step of a `calcfluxes.call` row to `flux`.
fn flux_step(r: &Row, set: &mut FluxSettings, dom: &OutputDomain, flux: &mut FluxGrid) {
    let a = &r.args;
    set.output_for_each_release = a[1] == 1.0;
    set.domain_filling = a[2] == 1.0;
    let step = ParticleStep {
        old: [a[5], a[6], a[7]],
        new: [a[8], a[9], a[10]],
        npoint: a[4] as usize - 1,
        nage: a[3] as usize - 1,
    };
    calcfluxes(&step, &[a[11], a[12]], set, dom, flux).expect("indices in range");
}

fn flux_of_call(p: Precision, id: usize) -> FluxGrid {
    let (mut set, dom) = flux_setup(p);
    let mut flux = empty_flux(&dom, set.outheight.len());
    let r = rows(p)
        .iter()
        .find(|r| r.function == "calcfluxes.call" && r.args[0] as usize == id)
        .expect("call row");
    flux_step(r, &mut set, &dom, &mut flux);
    flux
}

fn flux_accumulated(p: Precision) -> FluxGrid {
    let (mut set, dom) = flux_setup(p);
    let mut flux = empty_flux(&dom, set.outheight.len());
    for r in rows(p).iter().filter(|r| r.function == "calcfluxes.call") {
        flux_step(r, &mut set, &dom, &mut flux);
    }
    flux
}

/// Value at a `(id; dir; ix; jy; kz; k; kp; nage)` cell row (1-based
/// direction, level, species, release, age class as upstream).
fn flux_cell(flux: &FluxGrid, a: &[f64]) -> f64 {
    let u = |i: usize| a[i] as usize;
    flux.values[flux.index(u(1) - 1, u(2), u(3), u(4) - 1, u(5) - 1, u(6) - 1, u(7) - 1)]
}

/// Masses are dyadic and every sum is exact in `f32`: bit-exact at both
/// precisions is expected.
const FLUX: Bounds = Bounds::rel(0.0, 0.0);

#[test]
fn calcfluxes_single_steps() {
    let fx = fixtures();
    // The number of non-zero cells per call: with the per-cell check below,
    // this makes the port's set of non-zero cells equal upstream's.
    check_group(
        fx,
        "calcfluxes.call",
        FLUX,
        |_, _| true,
        |r, p| {
            let f = flux_of_call(p, r.args[0] as usize);
            Some(vec![f.values.iter().filter(|&&v| v != 0.0).count() as f64])
        },
    );
    check_group(
        fx,
        "calcfluxes.cell",
        FLUX,
        |_, _| true,
        |r, p| {
            Some(vec![flux_cell(
                &flux_of_call(p, r.args[0] as usize),
                &r.args,
            )])
        },
    );
}

#[test]
fn calcfluxes_accumulated() {
    for p in [Precision::Real4, Precision::Real8] {
        let f = flux_accumulated(p);
        let total: Vec<&Row> = rows(p)
            .iter()
            .filter(|r| r.function == "calcfluxes.total")
            .collect();
        assert_eq!(
            f.values.iter().filter(|&&v| v != 0.0).count(),
            total.len(),
            "{}: non-zero cell count",
            p.label()
        );
    }
    check_group(
        fixtures(),
        "calcfluxes.total",
        FLUX,
        |_, _| true,
        |r, p| Some(vec![flux_cell(&flux_accumulated(p), &r.args)]),
    );
}

/// The cyclic-boundary branch is exercised and records nothing (upstream's
/// `real(nxmin1)-1.e5`): every call whose zonal step is at least `nx/2`
/// adds no zonal flux.
#[test]
fn calcfluxes_wrapping_steps_record_no_zonal_flux() {
    for p in [Precision::Real4, Precision::Real8] {
        let rs = rows(p);
        let (set, _) = flux_setup(p);
        let wraps: Vec<usize> = rs
            .iter()
            .filter(|r| r.function == "calcfluxes.call")
            .filter(|r| (r.args[5] - r.args[8]).abs() >= set.nx as f64 / 2.0)
            .map(|r| r.args[0] as usize)
            .collect();
        assert!(
            wraps.len() >= 10,
            "sweep has {} wrapping steps",
            wraps.len()
        );
        for id in &wraps {
            let zonal = rs.iter().any(|r| {
                r.function == "calcfluxes.cell"
                    && r.args[0] as usize == *id
                    && (r.args[1] == 1.0 || r.args[1] == 2.0)
            });
            assert!(
                !zonal,
                "{}: upstream recorded a zonal flux for wrapping call {id}",
                p.label()
            );
            let f = flux_of_call(p, *id);
            for (i, v) in f.values.iter().enumerate() {
                if i % 6 == WEST_TO_EAST || i % 6 == EAST_TO_WEST {
                    assert_eq!(*v, 0.0);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// fluxoutput.

/// The flux field upstream's fluxoutput call `id` saw.
fn fluxoutput_input(p: Precision, id: usize) -> FluxGrid {
    let (set, dom) = flux_setup(p);
    let mut f = empty_flux(&dom, set.outheight.len());
    if id == 1 {
        for r in rows(p).iter().filter(|r| r.function == "calcfluxes.total") {
            let a = &r.args;
            let u = |i: usize| a[i] as usize;
            let k = f.index(u(1) - 1, u(2), u(3), u(4) - 1, u(5) - 1, u(6) - 1, u(7) - 1);
            f.values[k] = r.outs[0];
        }
    } else {
        // The driver's synth_flux, 1-based i, kz, k, kp, na.
        let (nx, ny) = (dom.numxgrid, dom.numygrid);
        for na in 1..=2usize {
            for kp in 1..=2usize {
                for k in 1..=2usize {
                    for kz in 1..=f.nz {
                        for jy in 0..ny {
                            for ix in 0..nx {
                                for i in 1..=6usize {
                                    let lin = ix + nx * (jy + ny * (kz - 1));
                                    let v = if k == 1 && na == 1 {
                                        0.25 * (1 + (lin * 7 + i + kp) % 13) as f64
                                    } else if k == 2 && na == 1 {
                                        if (lin * 5 + i) % 192 < 30 {
                                            0.5 * (1 + (lin + i) % 5) as f64
                                        } else {
                                            0.0
                                        }
                                    } else if k == 1 && na == 2 {
                                        if kp == 1 && (lin * 11 + i) % 192 < 44 {
                                            2.0 * (1 + lin % 3) as f64
                                        } else {
                                            0.0
                                        }
                                    } else if (lin + i) % 2 == 0 {
                                        0.125 * (((lin * 3 + i) % 9) as f64 - 2.0)
                                    } else {
                                        0.0
                                    };
                                    let idx = f.index(i - 1, ix, jy, kz - 1, k - 1, kp - 1, na - 1);
                                    f.values[idx] = v;
                                }
                            }
                        }
                    }
                }
            }
        }
        for r in rows(p).iter().filter(|r| r.function == "setup.fluxcheck") {
            let a = &r.args;
            let u = |i: usize| a[i] as usize;
            let idx = f.index(u(0) - 1, u(1), u(2), u(3) - 1, u(4) - 1, u(5) - 1, u(6) - 1);
            assert_eq!(f.values[idx], r.outs[0], "synthetic flux mismatch at {a:?}");
        }
        let n = f.values.iter().filter(|&&v| v != 0.0).count();
        assert_eq!(n as f64, find(rows(p), "setup.fluxcount").outs[0]);
        assert!(
            f.values.iter().any(|&v| v < 0.0),
            "the field carries negatives"
        );
    }
    f
}

/// The cell geometry upstream's own `outgrid_init` produced for `cfg` (the
/// `outgrid_init.cell` rows; every column of a row is identical upstream, so
/// the printed `ix = 0` value fills the row). `fluxoutput` takes the areas as
/// INPUT: feeding it upstream's values isolates its arithmetic from the area
/// computation, which `outgrid_init_cell_geometry` verifies on its own (and
/// which at real(4) carries the conditioning error documented there).
fn fixture_geometry(p: Precision, cfg: usize) -> CellGeometry {
    let mut g = geometry(p, cfg);
    for r in rows(p)
        .iter()
        .filter(|r| r.function == "outgrid_init.cell" && r.args[0] as usize == cfg)
    {
        let jy = r.args[2] as usize;
        let nz = g.numzgrid;
        for ix in 0..g.numxgrid {
            let i2 = g.idx2(ix, jy);
            g.area[i2] = r.outs[0];
            for kz in 0..nz {
                let i3 = g.idx3(ix, jy, kz);
                g.volume[i3] = r.outs[1 + kz];
                g.areaeast[i3] = r.outs[1 + nz + kz];
                g.areanorth[i3] = r.outs[1 + 2 * nz + kz];
            }
        }
    }
    if p == Precision::Real8 {
        assert_eq!(
            g,
            geometry(p, cfg),
            "real(8) geometry is the port's, bit for bit"
        );
    }
    g
}

fn fluxoutput_of(p: Precision, id: usize) -> (FluxOutput, FluxGrid) {
    let mut flux = fluxoutput_input(p, id);
    let r = rows(p)
        .iter()
        .find(|r| r.function == "fluxoutput.file" && r.args[0] as usize == id)
        .expect("fluxoutput.file row");
    let itime = r.args[1] as i64;
    let bdate = find(rows(p), "setup.bdate").args[0];
    let geo = fixture_geometry(p, 9);
    let out = fluxoutput(itime, bdate, 3600.0, &geo, &mut flux);
    (out, flux)
}

fn fluxoutputs() -> &'static HashMap<(u8, usize), (FluxOutput, FluxGrid)> {
    static S: OnceLock<HashMap<(u8, usize), (FluxOutput, FluxGrid)>> = OnceLock::new();
    S.get_or_init(|| {
        let mut m = HashMap::new();
        for (t, p) in [(4u8, Precision::Real4), (8u8, Precision::Real8)] {
            for id in [1, 2] {
                m.insert((t, id), fluxoutput_of(p, id));
            }
        }
        m
    })
}

fn fo(p: Precision, id: f64) -> &'static (FluxOutput, FluxGrid) {
    let t = match p {
        Precision::Real4 => 4u8,
        Precision::Real8 => 8u8,
    };
    &fluxoutputs()[&(t, id as usize)]
}

/// Upstream's file order of the six directions (position 1..6).
const FILE_ORDER: [usize; 6] = [
    EAST_TO_WEST,
    WEST_TO_EAST,
    SOUTH_TO_NORTH,
    NORTH_TO_SOUTH,
    UPWARD,
    DOWNWARD,
];

/// The block of `(id; k; kp; nage; d)` (1-based, `d` a file position).
fn block(p: Precision, a: &[f64]) -> &'static FluxLayout {
    let (out, _) = fo(p, a[0]);
    let dir = FILE_ORDER[a[4] as usize - 1];
    &out.blocks
        .iter()
        .find(|b| {
            b.ks == a[1] as usize - 1
                && b.kp == a[2] as usize - 1
                && b.nage == a[3] as usize - 1
                && b.dir == dir
        })
        .expect("block")
        .layout
}

/// `1e12*flux/area/outstep`: one rounding per operation in each build.
const FLUXOUT: Bounds = Bounds::rel(0.0, 1e-5);

#[test]
fn fluxoutput_records() {
    let fx = fixtures();
    check_group(
        fx,
        "fluxoutput.file",
        Bounds::rel(0.0, 0.0),
        |_, _| true,
        |r, p| {
            let (out, _) = fo(p, r.args[0]);
            assert_eq!(out.file_name().len(), "grid_flux_".len() + 14);
            Some(vec![
                (out.date / 10000) as f64,
                (out.date % 10000) as f64,
                out.time as f64,
            ])
        },
    );
    check_group(
        fx,
        "fluxoutput.itime",
        Bounds::rel(0.0, 0.0),
        |_, _| true,
        |r, p| Some(vec![fo(p, r.args[0]).0.itime as f64]),
    );
    check_group(
        fx,
        "fluxoutput.block",
        Bounds::rel(0.0, 0.0),
        |_, _| true,
        |r, p| {
            Some(match block(p, &r.args) {
                FluxLayout::Sparse(v) => vec![1.0, v.len() as f64],
                FluxLayout::Dense(v) => vec![2.0, v.len() as f64],
            })
        },
    );
    check_group(
        fx,
        "fluxoutput.sparse",
        FLUXOUT,
        |_, _| true,
        |r, p| {
            let FluxLayout::Sparse(v) = block(p, &r.args) else {
                return Some(vec![f64::NAN]);
            };
            let idx = r.args[5] as i64;
            Some(vec![v
                .iter()
                .find(|(i, _)| *i == idx)
                .map_or(f64::NAN, |&(_, x)| x)])
        },
    );
    check_group(
        fx,
        "fluxoutput.dense",
        FLUXOUT,
        |_, _| true,
        |r, p| {
            let FluxLayout::Dense(v) = block(p, &r.args) else {
                return Some(vec![f64::NAN; r.outs.len()]);
            };
            let ny = r.outs.len();
            let nx = geometry(p, 9).numxgrid;
            let (kz, ix) = (r.args[5] as usize - 1, r.args[6] as usize);
            let at = (kz * nx + ix) * ny;
            Some(v[at..at + ny].to_vec())
        },
    );
    // The port's blocks come in upstream's file order.
    for p in [Precision::Real4, Precision::Real8] {
        for id in [1.0, 2.0] {
            let want: Vec<(usize, usize, usize, usize)> = rows(p)
                .iter()
                .filter(|r| r.function == "fluxoutput.block" && r.args[0] == id)
                .map(|r| {
                    let a = &r.args;
                    let u = |i: usize| a[i] as usize - 1;
                    (u(1), u(2), u(3), FILE_ORDER[u(4)])
                })
                .collect();
            let got: Vec<(usize, usize, usize, usize)> = fo(p, id)
                .0
                .blocks
                .iter()
                .map(|b| (b.ks, b.kp, b.nage, b.dir))
                .collect();
            assert_eq!(got, want, "{}: block order of call {id}", p.label());
        }
    }
    check_group(
        fx,
        "fluxoutput.reset",
        Bounds::rel(0.0, 0.0),
        |_, _| true,
        |r, p| {
            let (_, f) = fo(p, r.args[0]);
            Some(vec![f.values.iter().filter(|&&v| v != 0.0).count() as f64])
        },
    );
}

/// Both layouts, and the over-release-point counting quirk, are exercised:
/// call 2's (species 2, age class 1) blocks are dense although each release
/// point alone has fewer than a quarter of the cells positive.
#[test]
fn fluxoutput_layouts_are_exercised() {
    for p in [Precision::Real4, Precision::Real8] {
        let (out, _) = fo(p, 2.0);
        let dense =
            |b: &&changi::flexpart::fluxes::FluxBlock| matches!(b.layout, FluxLayout::Dense(_));
        assert!(out.blocks.iter().any(|b| !dense(&b)));
        let input = fluxoutput_input(p, 2);
        let cells = input.nx * input.ny * input.nz;
        for b in out.blocks.iter().filter(|b| b.ks == 1 && b.nage == 0) {
            assert!(dense(&b), "species 2, age class 1 must be dense");
            let positive = (0..input.nz)
                .flat_map(|kz| {
                    (0..input.ny).flat_map(move |jy| (0..input.nx).map(move |ix| (ix, jy, kz)))
                })
                .filter(|&(ix, jy, kz)| {
                    input.values[input.index(b.dir, ix, jy, kz, 1, b.kp, 0)] > 0.0
                })
                .count();
            assert!(
                4 * positive < cells,
                "each release point alone would be sparse"
            );
        }
        // Sparse blocks skip non-positive cells; dense ones keep negatives.
        assert!(out.blocks.iter().any(|b| match &b.layout {
            FluxLayout::Dense(v) => v.iter().any(|&x| x < 0.0),
            FluxLayout::Sparse(_) => false,
        }));
    }
}

// ---------------------------------------------------------------------------
// initial_cond_calc.

fn ic_mets() -> &'static [GriddedMet; 2] {
    static S: OnceLock<[GriddedMet; 2]> = OnceLock::new();
    S.get_or_init(|| {
        let r = find(rows(Precision::Real8), "setup.icmet");
        let nz = r.args[0] as usize;
        let mut met = GriddedMet::zeros(NXMAX, NYMAX, nz);
        met.height = r.args[1..=nz].to_vec();
        for m in 1..=2usize {
            for k in 1..=nz {
                for jy in 0..NYMAX {
                    for ix in 0..NXMAX {
                        let i3 = met.idx3(ix, jy, k - 1, m - 1);
                        met.rho[i3] = 1.25 - 0.0625 * k as f64
                            + ((37 * ix + 101 * jy + 53 * k + 17 * m) % 97) as f64 / 256.0;
                    }
                }
            }
        }
        for p in [Precision::Real4, Precision::Real8] {
            let h = find(rows(p), "setup.icmet");
            assert_eq!(h.args[1..=nz], met.height[..], "heights at {}", p.label());
            for c in rows(p).iter().filter(|r| r.function == "setup.rhocheck") {
                let a = &c.args;
                let i3 = met.idx3(
                    a[0] as usize,
                    a[1] as usize,
                    a[2] as usize - 1,
                    a[3] as usize - 1,
                );
                assert_eq!(met.rho[i3], c.outs[0], "rho formula mismatch at {a:?}");
            }
        }
        // memind(2) = 2 (slots in order) and memind(2) = 1 (swapped).
        let mut swapped = met.clone();
        met.memind = [0, 1];
        swapped.memind = [1, 0];
        [met, swapped]
    })
}

fn ic_setup(p: Precision) -> (InitCondSettings, OutputDomain) {
    let a = &find(rows(p), "setup.ic").args;
    (
        InitCondSettings {
            unit: InitCondUnit::Mass,
            output_for_each_release: false,
            domain_filling: false,
            outheight: a[8..12].to_vec(),
            dx: a[0],
            dy: a[1],
        },
        OutputDomain {
            numxgrid: a[2] as usize,
            numygrid: a[3] as usize,
            dxout: a[4],
            dyout: a[5],
            xoutshift: a[6],
            youtshift: a[7],
        },
    )
}

/// Apply the particle of an `initial_cond_calc.call` row.
fn ic_step(
    r: &Row,
    set: &mut InitCondSettings,
    dom: &OutputDomain,
    grid: &mut ConcentrationGrid,
) -> Result<(), InitCondError> {
    let a = &r.args;
    set.unit = if a[3] == 1.0 {
        InitCondUnit::Mass
    } else {
        InitCondUnit::MassMixingRatio
    };
    set.output_for_each_release = a[5] == 1.0;
    set.domain_filling = a[6] == 1.0;
    let met = &ic_mets()[if a[4] == 2.0 { 0 } else { 1 }];
    let particle = InitCondParticle {
        itra1: a[2] as i64,
        xtra1: a[7],
        ytra1: a[8],
        ztra1: a[9],
        npoint: a[10] as usize - 1,
    };
    initial_cond_calc(
        a[1] as i64,
        &particle,
        &[a[11], a[12]],
        set,
        Some(met),
        dom,
        grid,
    )
}

fn ic_grid(dom: &OutputDomain, nz: usize) -> ConcentrationGrid {
    ConcentrationGrid::zeros(dom.numxgrid, dom.numygrid, nz, 2, 2, 1, 1)
}

fn ic_of_call(p: Precision, id: usize) -> ConcentrationGrid {
    let (mut set, dom) = ic_setup(p);
    let mut g = ic_grid(&dom, set.outheight.len());
    let r = rows(p)
        .iter()
        .find(|r| r.function == "initial_cond_calc.call" && r.args[0] as usize == id)
        .expect("call row");
    ic_step(r, &mut set, &dom, &mut g).expect("upstream's sweep stays inside the arrays");
    g
}

fn ic_accumulated(p: Precision) -> ConcentrationGrid {
    let (mut set, dom) = ic_setup(p);
    let mut g = ic_grid(&dom, set.outheight.len());
    for r in rows(p)
        .iter()
        .filter(|r| r.function == "initial_cond_calc.call")
    {
        ic_step(r, &mut set, &dom, &mut g).expect("inside the arrays");
    }
    g
}

/// `(id; ix; jy; kz; ks; kp)`, level, species, release 1-based.
fn ic_cell(g: &ConcentrationGrid, a: &[f64]) -> f64 {
    let u = |i: usize| a[i] as usize;
    g.values[g.index(u(1), u(2), u(3) - 1, u(4) - 1, u(5) - 1, 0, 0)]
}

const IC: Bounds = Bounds::rel(0.0, 1e-5);

#[test]
fn initial_cond_calc_single_particles() {
    let fx = fixtures();
    check_group(
        fx,
        "initial_cond_calc.call",
        Bounds::rel(0.0, 0.0),
        |_, _| true,
        |r, p| {
            let g = ic_of_call(p, r.args[0] as usize);
            Some(vec![g.values.iter().filter(|&&v| v != 0.0).count() as f64])
        },
    );
    check_group(
        fx,
        "initial_cond_calc.cell",
        IC,
        |_, _| true,
        |r, p| Some(vec![ic_cell(&ic_of_call(p, r.args[0] as usize), &r.args)]),
    );
}

#[test]
fn initial_cond_calc_accumulated() {
    for p in [Precision::Real4, Precision::Real8] {
        let g = ic_accumulated(p);
        let n = rows(p)
            .iter()
            .filter(|r| r.function == "initial_cond_calc.total")
            .count();
        assert_eq!(
            g.values.iter().filter(|&&v| v != 0.0).count(),
            n,
            "{}",
            p.label()
        );
    }
    check_group(
        fixtures(),
        "initial_cond_calc.total",
        IC,
        |_, _| true,
        |r, p| Some(vec![ic_cell(&ic_accumulated(p), &r.args)]),
    );
}

/// Where upstream reads an unassigned level index the port refuses.
#[test]
fn initial_cond_calc_refuses_above_the_top_model_level() {
    let (mut set, dom) = ic_setup(Precision::Real8);
    set.unit = InitCondUnit::Mass;
    let mut g = ic_grid(&dom, set.outheight.len());
    let met = &ic_mets()[0];
    let top = *met.height.last().expect("levels");
    let particle = InitCondParticle {
        itra1: 0,
        xtra1: 9.0,
        ytra1: 2.25,
        ztra1: top,
        npoint: 0,
    };
    assert_eq!(
        initial_cond_calc(0, &particle, &[1.0, 1.0], &set, Some(met), &dom, &mut g),
        Err(InitCondError::AboveTopLevel)
    );
    assert!(g.values.iter().all(|&v| v == 0.0));
    set.unit = InitCondUnit::MassMixingRatio;
    // Above the top model level but below no output level: nothing added.
    assert_eq!(
        initial_cond_calc(0, &particle, &[1.0, 1.0], &set, None, &dom, &mut g),
        Ok(())
    );
}
