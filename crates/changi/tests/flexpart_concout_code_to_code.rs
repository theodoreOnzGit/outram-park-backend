// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::{concoutput, timemanager}`
// against upstream FLEXPART v10.4 (commit 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART stage "concout" — output conversion and the timemanager step
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_concout.f90` (built by
//! `dev/build_reference_concout.sh`) has two parts.
//!
//! **`concoutput`, `concoutput_nest`, `concoutput_surf`** are compiled
//! verbatim (with `mean_mod`, `caldate` and the modules) and **run**: each
//! writes its binary files into the build directory, and the driver reads
//! every file back record by record. So what is compared is exactly what
//! FLEXPART writes: the sparse-packed wet / dry / concentration (or
//! residence-time, or particle-count) / mixing-ratio fields of every
//! (species, release slot, age class), the `factor_drygrid` file and the
//! three receptor files. The module arrays the routines leave
//! (`densityoutgrid`, `densitydrygrid`, `factor_drygrid`, `factor3d`, and
//! the last slice's `grid`, `gridsigma`, `wetgrid`, `wetgridsigma`,
//! `drygrid`, `drygridsigma`) and the three uncertainty outputs are compared
//! too. Six switch configurations (forward/backward, `iout` 1, 2, 3, 5,
//! wet/dry on/off, `memind` both orders, `outnum` 0.5-3, two output-height
//! sets — one with a half-height exactly on a model level, one above the
//! top — and a ~1e-17 kg case that falls under `mean_mod`'s absolute
//! `eps`), each through all three routines, on a 5x4x3 mother and 4x3x3
//! nest output grid over an 8x7x6 met grid; output-cell centres and
//! receptors sit on `nint` ties and outside the met grid (clamps).
//!
//! `par_mod.f90` variants (configuration only, checked by the build): V0 as
//! shipped; V1 `maxspec=2, maxageclass=2, nclassunc=3`; V2 = V1 with
//! `lparticlecountoutput` (two cases). **The real(8) build also sets
//! `dep_prec=dp`: upstream does not compile with `-fdefault-real-8` and the
//! shipped `dep_prec=sp`** (no specific of the generic `mean` for a
//! `real(4)` sample and `real(8)` results; see `concoutput.rs`).
//!
//! **`timemanager`'s bookkeeping** exists only inline, so, as stage 0 did
//! for radioactive decay, the cited lines are copied **byte for byte** into
//! driver subroutines, and the build script checks them against upstream
//! before compiling: lines 264-299 (deposition decay), 468-499 (particle
//! splitting), 534-593 (before `advance`), 625-703 (after `advance`), and the
//! 37 lines of the output clock (118-121, 151, 264, 345-358, 360, 371-372,
//! 431-432, 452-457, 459, 468, 499-501, 509-513), where only the `call`
//! statements are replaced by recording. The routines those lines call
//! (`initialize`, `get_vdep_prob`, `get_wetscav`, `drydepokernel(_nest)`,
//! `initial_cond_calc`) are recording stubs: they are verified in their own
//! stages; what is under test is how timemanager uses them.
//!
//! Synthetic inputs are exact dyadic numbers built from integer indices, so
//! they are identical in both builds and here; `setup.fp` rows fingerprint
//! every input array in double precision and are checked first.
//!
//! ## Results
//!
//! Measured **2026-10-02**, upstream `3d7eebf`, 14 tests, all passing
//! (`cargo test --release -p changi --test flexpart_concout_code_to_code --
//! --nocapture` reprints it). Bound: real(8) **tol 0** (bit-exact required),
//! real(4) `1e-5` relative. "max rel dev" is over every output of every row.
//!
//! | Group | Rows | Outputs | vs `real8` | vs `real4` |
//! |---|---:|---:|---:|---:|
//! | `co.sparse` (every packed field of every file) | 906 | 18 981 | **0 (bit-exact)** | 2.91e-7 |
//! | `co.dense` (density, dry density, factors, last-slice mean/sigma) | 360 | 6 240 | **0 (bit-exact)** | 1.33e-7 |
//! | `co.recv` (receptor files) | 174 | 444 | **0 (bit-exact)** | 1.33e-7 |
//! | `co.unc` (`gridtotalunc`, wet, dry) | 28 | 84 | **0 (bit-exact)** | 3.41e-7 |
//! | `co.itime` | 84 | 84 | **0** | 0 |
//! | `tm.clock` (7 schedules) | 283 | 3 962 | **0** | 0 |
//! | `tm.decaydep` | 120 (real4: 96) | 1 920 | **0 (bit-exact)** | 1.26e-7 |
//! | `tm.pre` | 24 | 192 | **0** | 8.1e-9 |
//! | `tm.post` | 32 | 672 | **0 (bit-exact)** | 7.2e-8 |
//! | `tm.split` | 40 | 460 | **0** | 0 |
//!
//! Plus: all 370 input fingerprints match exactly at both precisions; every
//! file the driver expected exists and the port produces exactly the
//! fixture's blocks (`concoutput_record_counts`); the upstream reset
//! behaviour and the absolute-`eps` zero uncertainty are pinned.
//!
//! Interpretation: the translation, including the explicit `real(sp)`
//! roundings that survive a double-precision build (`wetgrid`, `drygrid`,
//! `gridtotal*`), the strict-inequality level search, the literal time slot
//! of `concoutput_surf`, the run-length packing and the clock's integer
//! arithmetic, is reproduced exactly on this sweep. Against the shipped
//! build the residual is FLEXPART's `real(4)` arithmetic, at most 3.4e-7.
//! One real(4) exclusion, by a criterion derived outside the comparison:
//! `tm.decaydep` case 5, where `exp(-108)` underflows `f32` (see that test).
//!
//! The suite is not vacuous: 28 mutations of the port (13 in
//! `concoutput`, 15 in `timemanager`) were each killed; the table is in the
//! stage report. Two first-pass survivors were sweep gaps and were closed:
//! `nclassunc` was a power of two (making `real(sp)` rounding of the mean
//! invisible; now 3), and no particle had `itime = 0` with `itramem /= 0`
//! (now a restart particle).
//!
//! ## What this does NOT establish
//!
//! Verification, not validation, on synthetic fields. The binary file
//! layout itself (record markers, file names) is FLEXPART's and is not
//! ported. Nothing here supports emergency response or dose assessment for
//! real populations.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use changi::flexpart::concentration::{ConcentrationGrid, DepositionGrid};
use changi::flexpart::concoutput::{
    concoutput, ConcOutput, ConcOutputSettings, DensityFields, OutputGrid, ReceptorInput, Routine,
    SparseField,
};
use changi::flexpart::timemanager::{
    decay_deposition_grid, deposition_decay_due, post_advance, pre_advance, split_particles,
    ClockSettings, OutputClock, PostAdvanceSettings, PreAdvanceSettings, SplitParticle,
    StepParticle, WetScavResult,
};
use common::{check_group, Bounds, Fixtures, Precision, Row};

const REAL4: &str = include_str!("data/flexpart_concout_real4.csv");
const REAL8: &str = include_str!("data/flexpart_concout_real8.csv");

fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| Fixtures::new(REAL4, REAL8, &["setup.fp"]))
}

/// Real(8): bit-exact. Real(4): 1e-5 relative.
const STRICT: Bounds = Bounds::rel(0.0, 1e-5);

// Grid sizes of the driver.
const NXO: usize = 5;
const NYO: usize = 4;
const NZO: usize = 3;
const NXN: usize = 4;
const NYN: usize = 3;
const NPT: usize = 2;
const NXM: usize = 8;
const NYM: usize = 7;
const NZM: usize = 6;

/// `par_mod` variant dimensions, from the `setup.variant` rows.
#[derive(Clone, Copy, Debug)]
struct Variant {
    maxspec: usize,
    maxageclass: usize,
    nclassunc: usize,
    particle_count: bool,
}

fn variant(v: usize) -> Variant {
    let rows = &fixtures().real8;
    let r = rows
        .iter()
        .find(|r| r.function == "setup.variant" && r.args[0] as usize == v)
        .expect("setup.variant row");
    Variant {
        maxspec: r.args[1] as usize,
        maxageclass: r.args[2] as usize,
        nclassunc: r.args[3] as usize,
        particle_count: r.args[4] != 0.0,
    }
}

// ---------------------------------------------------------------------------
// The driver's input formulas (flexpart_reference_concout.f90).
// ---------------------------------------------------------------------------

fn fmod(a: i64, b: i64) -> i64 {
    a % b
}

fn cscale(icase: i64) -> f64 {
    if icase == 3 {
        2f64.powi(-56)
    } else {
        2f64.powi(-30)
    }
}

/// `gval`, 1-based ks, kp, l, nage; kz = 0 for 2-D grids.
#[allow(clippy::too_many_arguments)]
fn gval(
    icase: i64,
    ix: i64,
    jy: i64,
    kz: i64,
    ks: i64,
    kp: i64,
    l: i64,
    nage: i64,
    ik: i64,
) -> f64 {
    let m = fmod(
        2 * ix + 3 * jy + 5 * kz + ks + 2 * kp + 3 * nage + icase + 4 * ik,
        7,
    );
    if m <= 1 {
        return 0.0;
    }
    let k = if m == 6 {
        7 + fmod(ix + jy + kz, 5)
    } else {
        1 + fmod(17 * l + 5 * ix + 3 * jy + kz + m + ik, 50)
    };
    let e = fmod(ix + jy + kz + ik, 5) - 2;
    let x = k as f64 / 64.0 * 2f64.powi(e as i32) * cscale(icase);
    if m != 6 && l == 2 {
        x * 2f64.powi(-24)
    } else {
        x
    }
}

fn rval(ix: usize, jy: usize, k: usize, islot: usize) -> f64 {
    1.25 - 0.09375 * (k as f64 - 1.0)
        + 0.0078125 * ((ix + 2 * jy) % 5) as f64
        + 0.03125 * (islot as f64 - 1.0)
}

fn rdval(ix: usize, jy: usize, k: usize, islot: usize) -> f64 {
    rval(ix, jy, k, islot) - 0.015625 * ((ix + jy) % 3) as f64
}

fn met() -> DensityFields {
    let mut rho = Vec::new();
    let mut rho_dry = Vec::new();
    for islot in 1..=2 {
        for k in 1..=NZM {
            for jy in 0..NYM {
                for ix in 0..NXM {
                    rho.push(rval(ix, jy, k, islot));
                    rho_dry.push(rdval(ix, jy, k, islot));
                }
            }
        }
    }
    DensityFields {
        nxmin1: NXM - 1,
        nymin1: NYM - 1,
        nz: NZM,
        xlon0: 0.0,
        ylat0: 10.0,
        dx: 1.0,
        dy: 0.5,
        height: vec![0.0, 50.0, 150.0, 400.0, 1000.0, 2500.0],
        rho,
        rho_dry,
        nslots: 2,
    }
}

fn mother() -> OutputGrid {
    let mut area = Vec::new();
    let mut volume = vec![0.0; NXO * NYO * NZO];
    for jy in 0..NYO {
        for ix in 0..NXO {
            area.push(65536.0 * (1.0 + 0.25 * ((ix + jy) % 4) as f64));
            for k in 1..=NZO {
                volume[ix + NXO * (jy + NYO * (k - 1))] =
                    2f64.powi(20) * (1.0 + 0.125 * ((ix + 2 * jy + k) % 5) as f64);
            }
        }
    }
    OutputGrid {
        numxgrid: NXO,
        numygrid: NYO,
        outlon0: -1.5,
        outlat0: 10.25,
        dxout: 1.5,
        dyout: 1.25,
        area,
        volume,
    }
}

fn nest() -> OutputGrid {
    let mut area = Vec::new();
    let mut volume = vec![0.0; NXN * NYN * NZO];
    for jy in 0..NYN {
        for ix in 0..NXN {
            area.push(16384.0 * (1.0 + 0.5 * ((2 * ix + jy) % 3) as f64));
            for k in 1..=NZO {
                volume[ix + NXN * (jy + NYN * (k - 1))] =
                    2f64.powi(18) * (1.0 + 0.25 * ((ix + jy + 2 * k) % 3) as f64);
            }
        }
    }
    OutputGrid {
        numxgrid: NXN,
        numygrid: NYN,
        outlon0: 0.5,
        outlat0: 10.5,
        dxout: 0.75,
        dyout: 0.5,
        area,
        volume,
    }
}

struct CaseInputs {
    gridunc: ConcentrationGrid,
    griduncn: ConcentrationGrid,
    wet: DepositionGrid,
    dry: DepositionGrid,
    wetn: DepositionGrid,
    dryn: DepositionGrid,
    receptors: ReceptorInput,
    xmass: Vec<f64>,
    /// Fingerprints in the driver's order (ids 11..17).
    fp: [f64; 7],
}

fn case_inputs(v: Variant, icase: i64) -> CaseInputs {
    let (ns, nc, na) = (v.maxspec, v.nclassunc, v.maxageclass);
    let mut gridunc = ConcentrationGrid::zeros(NXO, NYO, NZO, ns, NPT, nc, na);
    let mut griduncn = ConcentrationGrid::zeros(NXN, NYN, NZO, ns, NPT, nc, na);
    let mut wet = DepositionGrid::zeros(NXO, NYO, ns, NPT, nc, na);
    let mut dry = wet.clone();
    let mut wetn = DepositionGrid::zeros(NXN, NYN, ns, NPT, nc, na);
    let mut dryn = wetn.clone();
    let mut fp = [0.0; 7];
    for nage in 1..=na {
        for l in 1..=nc {
            for kp in 1..=NPT {
                for ks in 1..=ns {
                    let g = |ix: usize, jy: usize, kz: usize, ik: i64| {
                        gval(
                            icase,
                            ix as i64,
                            jy as i64,
                            kz as i64,
                            ks as i64,
                            kp as i64,
                            l as i64,
                            nage as i64,
                            ik,
                        )
                    };
                    for kz in 1..=NZO {
                        for jy in 0..NYO {
                            for ix in 0..NXO {
                                let x = g(ix, jy, kz, 1);
                                let i =
                                    gridunc.index(ix, jy, kz - 1, ks - 1, kp - 1, l - 1, nage - 1);
                                gridunc.values[i] = x;
                                fp[0] += x;
                            }
                        }
                        for jy in 0..NYN {
                            for ix in 0..NXN {
                                let x = g(ix, jy, kz, 4);
                                let i =
                                    griduncn.index(ix, jy, kz - 1, ks - 1, kp - 1, l - 1, nage - 1);
                                griduncn.values[i] = x;
                                fp[3] += x;
                            }
                        }
                    }
                    for jy in 0..NYO {
                        for ix in 0..NXO {
                            let i = wet.index(ix, jy, ks - 1, kp - 1, l - 1, nage - 1);
                            wet.values[i] = g(ix, jy, 0, 2);
                            dry.values[i] = g(ix, jy, 0, 3);
                            fp[1] += wet.values[i];
                            fp[2] += dry.values[i];
                        }
                    }
                    for jy in 0..NYN {
                        for ix in 0..NXN {
                            let i = wetn.index(ix, jy, ks - 1, kp - 1, l - 1, nage - 1);
                            wetn.values[i] = g(ix, jy, 0, 5);
                            dryn.values[i] = g(ix, jy, 0, 6);
                            fp[4] += wetn.values[i];
                            fp[5] += dryn.values[i];
                        }
                    }
                }
            }
        }
    }
    let nrec = 4;
    let mut creceptor = vec![0.0; nrec * ns];
    let mut xmass = vec![0.0; NPT * ns];
    for ks in 1..=ns {
        for i in 1..=nrec {
            let c = 0.125 * ((3 * i + ks + icase as usize) % 5) as f64 * 2f64.powi(-30);
            creceptor[(i - 1) + nrec * (ks - 1)] = c;
            fp[6] += c;
        }
        for kp in 1..=NPT {
            xmass[(kp - 1) + NPT * (ks - 1)] = 0.5 * kp as f64 + 0.25 * ks as f64;
        }
    }
    CaseInputs {
        gridunc,
        griduncn,
        wet,
        dry,
        wetn,
        dryn,
        receptors: ReceptorInput {
            x: vec![0.5, 3.25, 7.75, -0.625],
            y: vec![2.5, -0.5, 6.875, 3.5],
            creceptor,
        },
        xmass,
        fp,
    }
}

/// The `case` row of (variant, case).
fn case_row(p: Precision, v: usize, icase: usize) -> &'static Row {
    fixtures()
        .rows(p)
        .iter()
        .find(|r| r.function == "case" && r.args[0] as usize == v && r.args[1] as usize == icase)
        .expect("case row")
}

type Key = (Precision, usize, usize, usize);

thread_local! {
    static CACHE: RefCell<HashMap<(u8, usize, usize, usize), ConcOutput>> = RefCell::new(HashMap::new());
}

/// Run the port for (precision, variant, case, routine 1/2/3).
fn port(key: Key) -> ConcOutput {
    let (p, v, icase, routine) = key;
    let ck = (p as u8, v, icase, routine);
    if let Some(o) = CACHE.with(|c| c.borrow().get(&ck).cloned()) {
        return o;
    }
    let var = variant(v);
    let r = case_row(p, v, icase);
    let a = &r.args;
    // case: variant, icase, ldirect, iout, wetdep, drydep, outnum,
    // outheight(3), memind(1), memind(2), loutaver, itime, nspec, npt, nageclass
    let inp = case_inputs(var, icase as i64);
    let set = ConcOutputSettings {
        routine: match routine {
            1 => Routine::Concoutput,
            2 => Routine::Nest,
            _ => Routine::Surface,
        },
        ldirect: a[2] as i64,
        iout: a[3] as i32,
        wetdep: a[4] != 0.0,
        drydep: a[5] != 0.0,
        outnum: a[6],
        loutaver: a[12] as i64,
        memind2: a[11] as usize,
        particle_count_output: var.particle_count,
        outheight: a[7..10].to_vec(),
        weightmolar: [352.0, 131.0][..var.maxspec].to_vec(),
        xmass: inp.xmass.clone(),
    };
    let fwd = set.ldirect > 0;
    let m = met();
    let out = if routine == 2 {
        concoutput(
            &set,
            &m,
            &nest(),
            &inp.griduncn,
            (fwd && set.wetdep).then_some(&inp.wetn),
            (fwd && set.drydep).then_some(&inp.dryn),
            &inp.receptors,
        )
    } else {
        concoutput(
            &set,
            &m,
            &mother(),
            &inp.gridunc,
            (fwd && set.wetdep).then_some(&inp.wet),
            (fwd && set.drydep).then_some(&inp.dry),
            &inp.receptors,
        )
    }
    .expect("port refused a fixture case");
    CACHE.with(|c| c.borrow_mut().insert(ck, out.clone()));
    out
}

fn key(r: &Row, p: Precision) -> Key {
    (
        p,
        r.args[0] as usize,
        r.args[1] as usize,
        r.args[2] as usize,
    )
}

fn nxy(routine: usize) -> usize {
    if routine == 2 {
        NXN * NYN
    } else {
        NXO * NYO
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// The input fingerprints (sums of every synthetic array in double
/// precision) must match the driver's exactly: the port is fed what the
/// Fortran was fed.
#[test]
fn inputs_match_the_driver() {
    for p in [Precision::Real4, Precision::Real8] {
        let mut var: Option<Variant> = None;
        let mut n = 0;
        for r in fixtures().rows(p) {
            match r.function.as_str() {
                "setup.variant" => var = Some(variant(r.args[0] as usize)),
                "setup.fp" => {
                    let v = var.expect("variant first");
                    let (icase, id) = (r.args[0] as i64, r.args[1] as i64);
                    let want = r.outs[0];
                    let got = match (icase, id) {
                        (0, 0) => {
                            let m = met();
                            m.rho
                                .iter()
                                .zip(&m.rho_dry)
                                .map(|(a, b)| a + 2.0 * b)
                                .sum::<f64>()
                        }
                        (0, 1) => {
                            let (mo, ne) = (mother(), nest());
                            let mut s = 0.0;
                            for g in [&mo, &ne] {
                                let nxy = g.numxgrid * g.numygrid;
                                for c in 0..nxy {
                                    s += g.area[c];
                                    for k in 0..NZO {
                                        s += g.volume[c + nxy * k];
                                    }
                                }
                            }
                            s
                        }
                        (c, i) => case_inputs(v, c).fp[(i - 11) as usize],
                    };
                    assert_eq!(got, want, "{} setup.fp {:?}", p.label(), r.args);
                    n += 1;
                }
                _ => {}
            }
        }
        // The rho fingerprint sums in a different order than the driver
        // (slot, level, row, column — the same); exact either way here.
        println!("{} input fingerprints: {n} matched exactly", p.label());
    }
}

// ---------------------------------------------------------------------------
// concoutput*
// ---------------------------------------------------------------------------

/// `co.dense`: per level (what 1-4: density, dry density, factor_drygrid,
/// factor3d — V0) and the last slice's statistics (5-6: grid, gridsigma per
/// level; 7-10: wetgrid, wetgridsigma, drygrid, drygridsigma — V1).
///
/// Real(4): `gridsigma` comes from the one-pass variance
/// `sum(x^2) - sum(x)^2/n`, which cancels when the classes are close. The
/// synthetic classes are far apart (one is `2^-24` of the others) or exactly
/// equal (variance exactly 0 in both builds), so no output is
/// ill-conditioned in `f32` and the strict bound holds (measured 1.3e-7).
#[test]
fn concoutput_dense_arrays() {
    check_group(
        fixtures(),
        "co.dense",
        STRICT,
        |_, _| true,
        |r, p| {
            let o = port(key(r, p));
            let routine = r.args[2] as usize;
            let iw = r.args[3] as usize;
            let n = nxy(routine);
            let kz = (r.args[4] as usize).saturating_sub(1);
            let lvl = |v: &[f64]| v[n * kz..n * (kz + 1)].to_vec();
            let last = &o.slices.last().expect("slices").stats;
            Some(match iw {
                1 => lvl(&o.density),
                2 => lvl(&o.density_dry),
                3 => lvl(&o.factor_drygrid),
                4 => lvl(&o.factor3d),
                5 => lvl(&last.grid),
                6 => lvl(&last.gridsigma),
                7 => last
                    .wetgrid
                    .as_ref()?
                    .iter()
                    .map(|&x| f64::from(x))
                    .collect(),
                8 => last.wetgridsigma.clone()?,
                9 => last
                    .drygrid
                    .as_ref()?
                    .iter()
                    .map(|&x| f64::from(x))
                    .collect(),
                10 => last.drygridsigma.clone()?,
                _ => return None,
            })
        },
    );
}

/// `co.unc`: `gridtotalunc` (`real(sp)` in both builds), `wetgridtotalunc`,
/// `drygridtotalunc`.
#[test]
fn concoutput_uncertainty_totals() {
    check_group(
        fixtures(),
        "co.unc",
        STRICT,
        |_, _| true,
        |r, p| {
            let u = port(key(r, p)).uncertainty?;
            Some(vec![
                f64::from(u.gridtotalunc),
                u.wetgridtotalunc,
                u.drygridtotalunc,
            ])
        },
    );
}

/// `co.itime`: the time record opening each grid file.
#[test]
fn concoutput_file_time_records() {
    check_group(
        fixtures(),
        "co.itime",
        STRICT,
        |_, _| true,
        |r, p| {
            let c = case_row(p, r.args[0] as usize, r.args[1] as usize);
            // The port has no files; the record is the call's itime.
            let _ = port(key(r, p));
            Some(vec![c.args[13]])
        },
    );
}

fn block(f: &SparseField) -> Vec<f64> {
    let mut v = vec![f.indices.len() as f64];
    v.extend(f.indices.iter().map(|&i| i as f64));
    v.push(f.values.len() as f64);
    v.extend(f.values.iter().copied());
    v
}

/// `co.sparse`: every packed field of every file, compared record for
/// record: run count, run-start indices (exactly), value count, signed
/// values. A changed zero pattern or run boundary changes the record length
/// and fails the comparison outright.
#[test]
fn concoutput_sparse_records() {
    check_group(
        fixtures(),
        "co.sparse",
        STRICT,
        |_, _| true,
        |r, p| {
            let o = port(key(r, p));
            let var = variant(r.args[0] as usize);
            let file = r.args[3] as usize;
            let b = r.args[4] as usize;
            if file == 3 {
                return (b == 1).then(|| block(&o.factor_drygrid_sparse));
            }
            let ks = file / 100 - 1;
            let local = (b - 1) / 3;
            let (kp, nage) = (local / var.maxageclass, local % var.maxageclass);
            let s = o
                .slices
                .iter()
                .find(|s| s.ks == ks && s.kp == kp && s.nage == nage)?;
            let recs = if file % 100 == 1 {
                s.conc.as_ref()?
            } else {
                s.pptv.as_ref()?
            };
            Some(block(match (b - 1) % 3 {
                0 => &recs.wet,
                1 => &recs.dry,
                _ => &recs.field,
            }))
        },
    );
}

/// Every block the port produces must appear in the fixture (the check
/// above goes fixture -> port; this goes port -> fixture), and no file the
/// driver expected may be missing.
#[test]
fn concoutput_record_counts() {
    for p in [Precision::Real4, Precision::Real8] {
        let rows = fixtures().rows(p);
        assert!(
            !rows.iter().any(|r| r.function == "co.missing"),
            "a file was not written"
        );
        let cases: Vec<&Row> = rows.iter().filter(|r| r.function == "case").collect();
        for c in cases {
            let (v, icase) = (c.args[0] as usize, c.args[1] as usize);
            for routine in 1..=3 {
                let o = port((p, v, icase, routine));
                let count = |file: usize| {
                    rows.iter()
                        .filter(|r| {
                            r.function == "co.sparse"
                                && r.args[0] as usize == v
                                && r.args[1] as usize == icase
                                && r.args[2] as usize == routine
                                && r.args[3] as usize == file
                        })
                        .count()
                };
                let ns = variant(v).maxspec;
                for ks in 0..ns {
                    let per = o.slices.iter().filter(|s| s.ks == ks);
                    let nconc: usize = per.clone().filter(|s| s.conc.is_some()).count() * 3;
                    let nppt: usize = per.filter(|s| s.pptv.is_some()).count() * 3;
                    assert_eq!(
                        count(100 * (ks + 1) + 1),
                        nconc,
                        "{v}/{icase}/{routine} conc"
                    );
                    assert_eq!(
                        count(100 * (ks + 1) + 2),
                        nppt,
                        "{v}/{icase}/{routine} pptv"
                    );
                }
                assert_eq!(count(3), 1, "factor_drygrid");
            }
        }
    }
}

/// `co.recv`: receptor files. File 4 `receptor_conc` (record 0 = itime,
/// then one per species), 5 `receptor_pptv` (`-1`: nothing written), 6
/// `factor_dryreceptor`.
#[test]
fn concoutput_receptor_records() {
    check_group(
        fixtures(),
        "co.recv",
        STRICT,
        |_, _| true,
        |r, p| {
            let o = port(key(r, p));
            let c = case_row(p, r.args[0] as usize, r.args[1] as usize);
            let itime = c.args[13];
            let rec = o.receptors.as_ref()?;
            let (file, irec) = (r.args[3] as usize, r.args[4] as usize);
            Some(match (file, irec) {
                (4, 0) | (6, 0) => vec![itime],
                (5, 0) => vec![if rec.pptv.is_some() { itime } else { -1.0 }],
                (4, k) => rec.conc[k - 1].clone(),
                (5, k) => rec.pptv.as_ref()?[k - 1].clone(),
                (6, 1) => rec.factor_dry.clone(),
                _ => return None,
            })
        },
    );
}

/// Upstream's own reset (not ported; the port leaves it to the caller):
/// `concoutput` and `concoutput_surf` zero `gridunc` and `creceptor`,
/// `concoutput_nest` zeroes `griduncn` **and `creceptor`**.
#[test]
fn concoutput_reset_behaviour_is_as_documented() {
    for p in [Precision::Real4, Precision::Real8] {
        for r in fixtures()
            .rows(p)
            .iter()
            .filter(|r| r.function == "co.reset")
        {
            let routine = r.args[2] as usize;
            let (gridunc, griduncn, crec) = (r.outs[0], r.outs[1], r.outs[2]);
            assert_eq!(crec, 0.0, "creceptor zeroed by every routine");
            if routine == 2 {
                assert_eq!(griduncn, 0.0);
                assert!(gridunc > 0.0);
            } else {
                assert_eq!(gridunc, 0.0);
                assert!(griduncn > 0.0);
            }
        }
    }
}

/// `mean_mod`'s absolute `eps`: case 3 (masses ~1e-17 kg) has spread
/// classes but every `gridsigma` is exactly 0 in FLEXPART and in the port.
#[test]
fn tiny_masses_report_zero_uncertainty() {
    let r = fixtures()
        .real8
        .iter()
        .find(|r| {
            r.function == "co.dense" && r.args[0] == 1.0 && r.args[1] == 3.0 && r.args[3] == 6.0
        })
        .expect("V1 case 3 gridsigma row");
    assert!(r.outs.iter().all(|&s| s == 0.0));
    let grid = fixtures()
        .real8
        .iter()
        .find(|q| q.function == "co.dense" && q.args[..3] == r.args[..3] && q.args[3] == 5.0)
        .expect("grid row");
    assert!(grid.outs.iter().any(|&g| g > 0.0));
    let o = port((Precision::Real8, 1, 3, 1));
    assert!(o
        .slices
        .iter()
        .all(|s| s.stats.gridsigma.iter().all(|&x| x == 0.0)));
}

// ---------------------------------------------------------------------------
// timemanager
// ---------------------------------------------------------------------------

fn sched_row(p: Precision, v: usize, isched: usize) -> &'static Row {
    fixtures()
        .rows(p)
        .iter()
        .find(|r| {
            r.function == "tm.sched" && r.args[0] as usize == v && r.args[1] as usize == isched
        })
        .expect("tm.sched row")
}

/// `tm.clock`: one row per synchronisation time of seven schedules.
#[test]
fn timemanager_output_clock() {
    check_group(
        fixtures(),
        "tm.clock",
        STRICT,
        |_, _| true,
        |r, p| {
            let s = sched_row(p, r.args[0] as usize, r.args[1] as usize);
            let a = &s.args;
            let set = ClockSettings {
                ldirect: a[2] as i64,
                loutstep: a[3] as i64,
                loutaver: a[4] as i64,
                loutsample: a[5] as i64,
                itsplit: a[8] as i64,
                iout: a[9] as i32,
                dep: a[10] != 0.0,
            };
            let (lsynctime, ideltas) = (a[6] as i64, a[7] as i64);
            let want = r.args[2] as i64;
            let mut clock = OutputClock::new(set);
            let mut itime = 0;
            loop {
                let before = clock.outnum;
                let last = itime == ideltas;
                let ev = clock.step(itime, last);
                if itime == want {
                    return Some(vec![
                        f64::from(u8::from(ev.decay_deposition)),
                        f64::from(u8::from(ev.sample.is_some())),
                        ev.sample.unwrap_or(0.0),
                        ev.sample.map_or(0.0, |w| before + w),
                        f64::from(u8::from(ev.output.is_some())),
                        ev.output.unwrap_or(0.0),
                        f64::from(u8::from(ev.resample)),
                        f64::from(u8::from(ev.split)),
                        f64::from(u8::from(last)),
                        ev.ldeltat.unwrap_or(0) as f64,
                        clock.outnum,
                        clock.loutnext as f64,
                        clock.loutstart as f64,
                        clock.loutend as f64,
                    ]);
                }
                if last {
                    return None;
                }
                itime += lsynctime;
            }
        },
    );
}

/// `tm.decaydep`: the deposition grids after the decay block.
///
/// Real(4): case 5 (`decay = 0.01 s^-1` over 10 800 s) has
/// `exp(-108) = 1.2e-47`, below `f32`'s smallest subnormal: the shipped
/// build stores 0 where the port has ~1e-56 kg. Those rows are taken out of
/// real(4) scope by that criterion (`decay * outstep > 103.3`, where
/// `exp` underflows `f32`); real(8) covers them bit for bit.
#[test]
fn timemanager_deposition_decay() {
    let in_scope = |r: &Row, p: Precision| {
        if p == Precision::Real8 {
            return true;
        }
        let c = decay_case(p, r.args[0] as usize, r.args[1] as usize);
        let outstep = c.args[6];
        !c.args[7..].iter().any(|&d| d * outstep > 103.3)
    };
    check_group(fixtures(), "tm.decaydep", STRICT, in_scope, |r, p| {
        let v = r.args[0] as usize;
        let var = variant(v);
        let c = decay_case(p, v, r.args[1] as usize);
        let (itime, loutnext, ldirect, nested) = (
            c.args[2] as i64,
            c.args[3] as i64,
            c.args[4] as i64,
            c.args[5] != 0.0,
        );
        let outstep = c.args[6];
        let decay = &c.args[7..7 + var.maxspec];
        let mut inp = case_inputs(var, 1);
        if deposition_decay_due(true, itime, loutnext, ldirect) {
            decay_deposition_grid(&mut inp.wet, decay, outstep);
            decay_deposition_grid(&mut inp.dry, decay, outstep);
            if nested {
                decay_deposition_grid(&mut inp.wetn, decay, outstep);
                decay_deposition_grid(&mut inp.dryn, decay, outstep);
            }
        }
        let (iarr, ks, kp, l, nage) = (
            r.args[2] as usize,
            r.args[3] as usize - 1,
            r.args[4] as usize - 1,
            r.args[5] as usize - 1,
            r.args[6] as usize - 1,
        );
        let g = match iarr {
            1 => &inp.wet,
            2 => &inp.dry,
            3 => &inp.wetn,
            _ => &inp.dryn,
        };
        let start = g.index(0, 0, ks, kp, l, nage);
        Some(g.values[start..start + g.nx * g.ny].to_vec())
    });
}

fn decay_case(p: Precision, v: usize, idc: usize) -> &'static Row {
    fixtures()
        .rows(p)
        .iter()
        .find(|r| {
            r.function == "tm.decaycase" && r.args[0] as usize == v && r.args[1] as usize == idc
        })
        .expect("tm.decaycase row")
}

/// `tm.pre`: release slot, age class, `initialize` trigger, backward
/// scavenging initialisation (lines 534-593).
#[test]
fn timemanager_before_advance() {
    check_group(
        fixtures(),
        "tm.pre",
        STRICT,
        |_, _| true,
        |r, _| {
            let var = variant(r.args[0] as usize);
            let ns = var.maxspec;
            let a = &r.args;
            let ic = a[1] as usize;
            let (itime, itra1, itramem, npoint) =
                (a[2] as i64, a[3] as i64, a[4] as i64, a[5] as usize);
            let prob_rec = &a[12..12 + ns];
            let grfraction1 = a[12 + ns];
            let (zp1, zp2) = (a[13 + ns], a[14 + ns]);
            let lage: Vec<i64> = a[15 + ns..15 + ns + var.maxageclass]
                .iter()
                .map(|&x| x as i64)
                .collect();
            let mut drydepspec = vec![true; ns];
            drydepspec[0] = a[9] != 0.0;
            let set = PreAdvanceSettings {
                output_each_release: a[6] != 0.0,
                lage,
                drybkdep: a[7] != 0.0,
                wetbkdep: a[8] != 0.0,
                drydepspec,
            };
            let mut xmass1 = vec![0.75; ns];
            let mut xscav = vec![-1.0; ns];
            if ic == 11 {
                xscav[0] = a[10];
            }
            let ws = vec![
                WetScavResult {
                    wetscav: a[11],
                    grfraction1,
                };
                ns
            ];
            let out = pre_advance(
                &set,
                itime,
                itra1,
                itramem,
                npoint,
                &mut xmass1,
                &mut xscav,
                prob_rec,
                &ws,
                zp1,
                zp2,
            );
            let mut o = vec![
                (out.kp + 1) as f64,
                out.nage.map_or(var.maxageclass + 1, |n| n + 1) as f64,
                f64::from(u8::from(out.initialize)),
                out.vdep_calls as f64,
                out.wetscav_calls as f64,
            ];
            o.extend(xscav);
            o.extend(xmass1);
            Some(o)
        },
    );
}

/// `tm.post`: termination, decay, dry-deposition mass removal, `minmass`,
/// the `drydepokernel` calls and their arguments, the age limit,
/// `initial_cond_calc` calls (lines 625-703).
#[test]
fn timemanager_after_advance() {
    check_group(
        fixtures(),
        "tm.post",
        STRICT,
        |_, _| true,
        |r, _| {
            let var = variant(r.args[0] as usize);
            let ns = var.maxspec;
            let a = &r.args;
            let at = |i: usize| a[i];
            let base = 22;
            let xmass1_in = &a[base..base + ns];
            let prob = &a[base + ns..base + 2 * ns];
            let decay = a[base + 2 * ns..base + 3 * ns].to_vec();
            let drydepspec: Vec<bool> = a[base + 3 * ns..base + 4 * ns]
                .iter()
                .map(|&x| x != 0.0)
                .collect();
            let xmass_rel = &a[base + 4 * ns..base + 5 * ns];
            let set = PostAdvanceSettings {
                ldirect: at(19) as i64,
                lsynctime: at(12) as i64,
                decay,
                drydep: at(18) != 0.0,
                drydepspec,
                mdomainfill: at(16) as i32,
                mquasilag: at(17) as i32,
                nested_output: at(20) != 0.0,
                linit_cond: at(21) as i32,
                lage_max: at(13) as i64,
            };
            let itime = at(2) as i64;
            let mut p = StepParticle {
                itra1: itime,
                itramem: at(7) as i64,
                nclass: at(9) as usize,
                xtra1: at(10),
                ytra1: at(11),
                xmass1: xmass1_in.to_vec(),
            };
            const SENTINEL: f64 = -7.0;
            let mut drydeposit = vec![SENTINEL; ns];
            let out = post_advance(
                &set,
                &mut p,
                itime,
                at(3) as i32,
                prob,
                at(4) as i64,
                at(5) as usize,
                at(6) as usize,
                xmass_rel,
                at(15) as i64,
                &mut drydeposit,
            );
            let mut o = vec![p.itra1 as f64];
            o.extend(&p.xmass1);
            o.extend(&drydeposit);
            o.push(out.xmassfract.unwrap_or(-1.0));
            let mother = out.kernel_calls.iter().rfind(|c| !c.nest);
            let nestc = out.kernel_calls.iter().rfind(|c| c.nest);
            o.push(out.kernel_calls.iter().filter(|c| !c.nest).count() as f64);
            match mother {
                Some(c) => {
                    o.extend([c.nunc as f64, c.x, c.y, c.nage as f64, c.kp as f64]);
                    o.extend(&c.deposit);
                }
                None => {
                    o.extend([0.0; 5]);
                    o.extend(vec![SENTINEL; ns]);
                }
            }
            o.push(out.kernel_calls.iter().filter(|c| c.nest).count() as f64);
            match nestc {
                Some(c) => {
                    o.extend([c.nunc as f64, c.x, c.y]);
                    o.extend(&c.deposit);
                }
                None => {
                    o.extend([0.0; 3]);
                    o.extend(vec![SENTINEL; ns]);
                }
            }
            o.push(out.initial_cond_calls.len() as f64);
            o.push(out.initial_cond_calls.first().copied().unwrap_or(0) as f64);
            o.push(out.initial_cond_calls.get(1).copied().unwrap_or(0) as f64);
            Some(o)
        },
    );
}

/// `tm.split`: particle splitting (lines 468-499), including the backward
/// direction and the `maxpart` cap.
#[test]
fn timemanager_particle_splitting() {
    check_group(
        fixtures(),
        "tm.split",
        STRICT,
        |_, _| true,
        |r, p| {
            let v = r.args[0] as usize;
            let ic = r.args[1] as usize;
            let ns = variant(v).maxspec;
            let c = fixtures()
                .rows(p)
                .iter()
                .find(|q| {
                    q.function == "tm.splitcase"
                        && q.args[0] as usize == v
                        && q.args[1] as usize == ic
                })
                .expect("tm.splitcase");
            let (ldirect, itime, itsplit, np, maxpart) = (
                c.args[2] as i64,
                c.args[3] as i64,
                c.args[4] as i64,
                c.args[5] as usize,
                c.args[6] as usize,
            );
            let j0 = np.saturating_sub(4).max(1);
            let mut parts: Vec<SplitParticle> = (1..=np)
                .map(|j| {
                    let ji = j as i64;
                    let itramem = ldirect * 600 * (ji % 4);
                    let mut itrasplit = itramem + ldirect * 3600 * (1 + ji % 5);
                    if ic == 3 && j < j0 {
                        itrasplit = ldirect * 999_999;
                    }
                    SplitParticle {
                        itra1: itime,
                        itramem,
                        itrasplit,
                        idt: 60 + ji,
                        npoint: 1 + j % 2,
                        nclass: 1 + j % 3,
                        xtra1: 1.25 + 0.5 * (j % 7) as f64,
                        ytra1: 2.0 + 0.25 * (j % 3) as f64,
                        ztra1: 50.0 * (1 + j % 4) as f64,
                        uap: 0.5,
                        ucp: -0.25,
                        uzp: 0.125,
                        us: 1.0,
                        vs: 2.0,
                        ws: 0.5,
                        cbt: (j % 2) as i16,
                        xmass1: (1..=ns)
                            .map(|ks| 0.375 * ks as f64 + 0.0625 * (j % 5) as f64)
                            .collect(),
                    }
                })
                .collect();
            split_particles(&mut parts, itime, ldirect, itsplit, maxpart);
            let j = r.args[2] as usize;
            if parts.len() != r.args[3] as usize {
                return Some(vec![f64::NAN]);
            }
            let q = &parts[j - 1];
            let mut o = vec![
                q.itrasplit as f64,
                q.itramem as f64,
                q.itra1 as f64,
                q.idt as f64,
                q.npoint as f64,
                q.nclass as f64,
                q.xtra1,
                q.ytra1,
                q.ztra1,
                f64::from(q.cbt),
            ];
            o.extend(&q.xmass1);
            Some(o)
        },
    );
}
