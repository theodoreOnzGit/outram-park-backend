// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart::convection` and
// `changi::flexpart::convmix` against upstream FLEXPART v10.4 (commit 3d7eebf,
// GPL-3.0-or-later).
//
// Uses only changi, and reads its fixtures through `include_str!`, so there is
// no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART convective mixing (Emanuel `convect43c`, `calcmatrix`, `redist`, `convmix`), code-to-code
//!
//! ## Methodology
//!
//! `dev/flexpart_reference_convection.f90` links upstream's `convect43c.f90`
//! (`CONVECT`, `TLIFT`), `calcmatrix.f90`, `redist.f90`, `convmix.f90`,
//! `sort2.f90`, `qvsat.f90`, `ew.f90` and the modules they use, **verbatim**,
//! and drives them on synthetic soundings and fields written straight into
//! `conv_mod`/`com_mod`. Two substitutions, both outside the routines under
//! test: `par_mod.f90` is a copy with `maxnests=1, nxmaxn=12, nymaxn=12`
//! (FLEXPART's user configuration; shipped zeros would disable the nest
//! branch), and `random_mod` is `dev/random_mod_shim_convection.f90`, whose `ran3` serves
//! a queue of draws the driver chooses. Numerical Recipes' `ran3` and `sort2`
//! are not ported.
//!
//! Groups:
//!
//! - `convect`: 73 calls on soundings built to straddle the scheme's
//!   branches: every `IFLAG` (0 by moist-static-energy profile, by a parcel
//!   below 250 K, by a dry parcel and by stability at cloud base; 2 by an LCL
//!   above 200 hPa and by the LCL formula reaching 2000 hPa; 3; 1; 4), the
//!   line-617 return (`IFLAG = 1` with zero flux), deep, shallow, capped
//!   (cloud tops from level 4 to `NL`), cold-based and elevated (`NK > 1`)
//!   convection, the `GOTO 405` skip of the precipitating downdraft, time
//!   steps 2 % below the CFL threshold of level 1 and of a level above, a
//!   level inside the 0.949 p(1) downdraft ramp, a flux small enough
//!   (`4.5e-10`) for `NCONVTOP`'s `EPSILON` to matter, and `NL = 3`; the
//!   cloud-base mass flux is chained between calls as FLEXPART chains it. Every output: `IFLAG`, `CBMF`, `PRECIP`, `WD`,
//!   `TPRIME`, `QPRIME`, `NCONVTOP`, `FT`, `FQ`, `SUB` and the full `FMASS`
//!   matrix. The driver sets `NCONVTOP = 777` before every call, so the rows
//!   record that upstream leaves it unassigned on early returns; the port
//!   returns `None` there and the test requires upstream's sentinel.
//! - `calcmatrix`: both meteorological formats on deep, shallow, stable,
//!   CFL and marginal columns, with remembered fluxes; replayed in sequence on
//!   one state so stale `conv_mod` values are reproduced, not tolerated.
//! - `redist`: 736 single-particle calls on three convecting columns,
//!   sweeping height (surface to above the domain and the model top) × draw
//!   (including exactly 0 and 1) × direction (forward, backward with negative
//!   `lsynctime`); calls that reuse the `SAVE`d `uvzlev` of the previous
//!   column; an inversion whose half-level virtual-temperature steps (0.028 K,
//!   0.231 K) straddle the `|dTv| > 0.2` switch; a 2 m temperature sweep
//!   across the same switch for the first step; and subsidence long enough to
//!   push particles below ground (reflected).
//! - `convmix`: four successive calls on a 5 × 4 mother grid and a 5 × 5 nest
//!   with 60 particles (ECMWF forward, again with `memind` swapped, GFS,
//!   ECMWF backward), sharing `cbaseflux` and `conv_mod` between calls.
//! - `sort2`: the permutation upstream's quicksort produces.
//!
//! ### Identical inputs in both builds
//!
//! The driver computes every synthetic input in double precision from
//! `real(4)`-representable parameters and rounds it once to `real(4)`, so the
//! two builds see **bit-identical inputs** (checked: the only argument
//! differences between the fixtures are the five chained cloud-base fluxes,
//! which carry the previous call's result). The real(4)-vs-real(8) distance
//! is then upstream's own arithmetic precision and nothing else, which is what
//! makes the precision-spread attribution below exact rather than heuristic.
//!
//! ### Random draws and the sort order
//!
//! Upstream consumes one `ran3` draw per particle that is inside the
//! convective domain, in the order `sort2` leaves the particles. The port's
//! stable sort visits the **same columns in the same order** with the **same
//! particles** in each (checked: the sorted keys are identical), but `sort2`
//! is an unstable quicksort, so within a column the particle order can differ
//! (measured in `sort2_permutation_vs_stable_sort`). To compare end to end
//! with distinct draws, the test reads upstream's permutation (the driver
//! calls `sort2` on the same keys and prints it), works out which draw
//! upstream gave each particle, and feeds the port the same draw per particle
//! in its own order. Which particles draw at all does not depend on the draw
//! values; the test establishes it with a first port pass.
//!
//! ## Results
//!
//! Taken 2026-10-02, upstream `3d7eebf`, gfortran `-O2`, 2 234 fixture rows.
//! Re-print with `cargo test --release -p changi --test
//! flexpart_convection_code_to_code -- --nocapture`.
//!
//! | Group | Rows | vs real(8) | vs real(4) (max rel dev; outputs attributed to upstream's f32 spread) |
//! |---|---:|---|---|
//! | `convect` | 73 | 3 745/3 745 bit-exact | 2.05 (an `FT` of 1e-10 K/s); 1 085 outputs, row 73 out of scope |
//! | `convect.fmass` | 765 | 11 475/11 475 bit-exact | 2.66e-3; 744 outputs |
//! | `calcmatrix` | 21 | 1 428/1 428 bit-exact | 1.47e-3 (`cbmf_out`); 13 outputs |
//! | `calcmatrix.fmassfrac` | 188 | 2 772/2 772 bit-exact | 1.50e-3; 396 outputs |
//! | `redist` | 736 | 2 944/2 944 bit-exact | 5.2e3 (a branch flip); 46 outputs, 6 `ipconv` flips |
//! | `convmix` | 4 | 424/424 bit-exact | 4.37e-4 (`cbaseflux`); 55 outputs; all 240 particle heights within 1e-5 |
//!
//! **Interpretation.** Against real(8) the port is bit-exact on every one of
//! the 22 788 outputs, so the translation is exact on this sweep, including the
//! stale-state behaviour. Against FLEXPART as shipped, the scheme is
//! pervasively ill-conditioned in `f32`: the cloud-base mass-flux increment is
//! `0.0025*DTMA`, with `DTMA = TVPPLCL - TVAPLCL + DTMAX + DTPBL` a difference
//! of ~300 K temperatures giving ~0.1 K (cancellation ~3e3, so ~2e-4 relative
//! in `f32`), and every mass flux is proportional to it; the entrainment
//! shape adds `|TV - TVP|` and `H - HP` differences of the same kind. With
//! identical inputs, the shipped build's departure from its own real(8)
//! build is up to 2.7e-3 in `FMASS`, 1.5e-3 in `fmassfrac`; in `redist` six
//! particles change destination level because a draw sits within `f32`
//! rounding of a cumulative mass fraction. The outputs NOT listed in a
//! spread rule (`IFLAG`, `NCONVTOP`, `lconv`, the `calcmatrix` column
//! diagnostics, `ktop`, draw counts, every `convmix` particle height) hold
//! the plain 1e-5 bound against real(4).
//!
//! ## What this does NOT establish
//!
//! Verification, not validation, on synthetic soundings: it shows the Rust
//! computes what the Fortran computes. It says nothing about whether Emanuel's
//! scheme, or FLEXPART's use of it, reproduces observed convective transport.
//! Per the crate scope limit, nothing here supports emergency response or dose
//! assessment for real populations.

mod common;

use std::collections::HashMap;
use std::sync::OnceLock;

use changi::flexpart::convection::{convect, ConvectSounding};
use changi::flexpart::convmix::{
    calcmatrix, convmix, convmix_column_keys, redist, sort_particles_by_column, ConvMetFormat,
    ConvMixMet, ConvMixNest, ConvMixParams, ConvMod, ConvParticles, HybridCoefficients,
};
use common::{check_group, Bounds, Fixtures, Precision, Real4Rule, Row};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_convection_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_convection_real8.csv");

/// `GRIBFILE_CENTRE_ECMWF` / `_NCEP` as the driver prints them.
const ECMWF: f64 = 2.0;
const NUVZ: usize = 17;
const NCONVLEV: usize = 15;
/// The sentinel the driver writes into `nconvtop` before each `convect` call.
const SENTINEL: f64 = 777.0;

/// `start, start+1, ..., start+N-1`, for the column lists below.
const fn cols<const N: usize>(start: usize) -> [usize; N] {
    let mut a = [0; N];
    let mut i = 0;
    while i < N {
        a[i] = start + i;
        i += 1;
    }
    a
}

/// `convect` outputs attributed to upstream's single precision in the real(4)
/// fixture: `CBMF`, `PRECIP`, `WD`, `TPRIME`, `QPRIME` (columns 1-5) and the
/// `FT`, `FQ`, `SUB` profiles (columns 7-51 for `NL = 14`). `IFLAG` (0) and
/// `NCONVTOP` (6) are held strictly. See [`convect_matches_flexpart`].
static CONVECT_SPREAD: [usize; 50] = {
    let a: [usize; 5] = cols(1);
    let b: [usize; 45] = cols(7);
    let mut c = [0; 50];
    let mut i = 0;
    while i < 5 {
        c[i] = a[i];
        i += 1;
    }
    while i < 50 {
        c[i] = b[i - 5];
        i += 1;
    }
    c
};
/// `calcmatrix`: only `cbmf_out` (column 1); `lconv`, `nconvtop` and the
/// column diagnostics are held strictly.
static CALCMATRIX_SPREAD: [usize; 1] = [1];
/// `redist`: the new height and `ipconv` (columns 0-1); `ktop` and the draw
/// count are held strictly.
static REDIST_SPREAD: [usize; 2] = [0, 1];
/// `convmix`: the mother and nest `cbaseflux` (columns 61-105). The 60
/// particle heights and the draw count are held strictly.
static CONVMIX_SPREAD: [usize; 45] = cols(61);

/// Real(8) bound 1e-13 (the port is in fact bit-exact), real(4) 1e-5 with
/// upstream's own spread admitted on the listed columns only.
const fn spread_on(c: &'static [usize]) -> Bounds {
    Bounds {
        tol8: 1e-13,
        floor8: 0.0,
        tol4: 1e-5,
        floor4: 0.0,
        rule: Real4Rule::PrecisionSpreadOn(c),
    }
}

/// As [`spread_on`] for every column (groups whose every output is
/// proportional to the cloud-base mass flux).
const SPREAD_ALL: Bounds = Bounds {
    tol8: 1e-13,
    floor8: 0.0,
    tol4: 1e-5,
    floor4: 0.0,
    rule: Real4Rule::PrecisionSpread,
};

fn fx() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| Fixtures::new(FIXTURE_REAL4, FIXTURE_REAL8, &["convmix.particle"]))
}

fn find<'a>(rows: &'a [Row], name: &str, id: f64) -> &'a Row {
    rows.iter()
        .find(|r| r.function == name && r.args[0] == id)
        .unwrap_or_else(|| panic!("no `{name}` row with id {id}"))
}

fn hybrid(rows: &[Row]) -> HybridCoefficients {
    let r = rows
        .iter()
        .find(|r| r.function == "hybrid")
        .expect("hybrid row");
    let n = r.args[0] as usize;
    let o = &r.outs;
    HybridCoefficients {
        akz: o[0..n].to_vec(),
        bkz: o[n..2 * n].to_vec(),
        akm: o[2 * n..3 * n].to_vec(),
        bkm: o[3 * n..4 * n].to_vec(),
    }
}

fn format_of(code: f64) -> ConvMetFormat {
    if code == ECMWF {
        ConvMetFormat::Ecmwf
    } else {
        ConvMetFormat::Gfs
    }
}

// ── convect ───────────────────────────────────────────────────────────────────

fn sounding(rows: &[Row], id: f64) -> (usize, ConvectSounding) {
    let s = find(rows, "convect.sounding", id);
    let nl = s.args[1] as usize;
    let n1 = nl + 1;
    let o = &s.outs;
    (
        nl,
        ConvectSounding {
            t: o[0..n1].to_vec(),
            q: o[n1..2 * n1].to_vec(),
            qs: o[2 * n1..3 * n1].to_vec(),
            p_hpa: o[3 * n1..4 * n1].to_vec(),
            ph_hpa: o[4 * n1..5 * n1].to_vec(),
        },
    )
}

fn run_convect(r: &Row, p: Precision) -> changi::flexpart::convection::ConvectOutput {
    let rows = fx().rows(p);
    let id = r.args[0];
    let call = find(rows, "convect", id);
    let (nl, s) = sounding(rows, id);
    convect(&s, nl, call.args[2], call.args[3])
}

fn eval_convect(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let out = run_convect(r, p);
    let mut o = vec![
        f64::from(out.iflag.code()),
        out.cbmf,
        out.precip,
        out.wd,
        out.tprime,
        out.qprime,
        // None = upstream leaves NCONVTOP unassigned: it must still hold the
        // driver's sentinel.
        out.nconvtop.map_or(r.args[4], |n| n as f64),
    ];
    o.extend(&out.ft);
    o.extend(&out.fq);
    o.extend(&out.sub);
    Some(o)
}

fn eval_convect_fmass(r: &Row, p: Precision) -> Option<Vec<f64>> {
    let out = run_convect(r, p);
    let i = r.args[1] as usize;
    Some((1..=out.n).map(|j| out.fmass(i, j)).collect())
}

/// `convect`: every scalar output, `NCONVTOP`, and the `FT`, `FQ`, `SUB`
/// profiles, on 47 soundings.
#[test]
fn convect_matches_flexpart() {
    check_group(
        fx(),
        "convect",
        spread_on(&CONVECT_SPREAD),
        convect_in_scope,
        eval_convect,
    );
}

/// Call 73 is out of the real(4) scope, and only there. Its remembered flux
/// was chosen to cancel the (negative) mass-flux increment to within
/// `CBMF = 4.48e-10` (real(8) and the port agree bit for bit), so that
/// `FMASS` falls between 1e-20 and 1e-8 and `NCONVTOP`'s `EPSILON` matters.
/// That residue is below what `0.7*CBMFOLD + 0.0025*DTMA` resolves in `f32`:
/// the shipped build clamps `CBMF` to exactly 0 and so reports `NCONVTOP = 2`
/// (a threshold decided by rounding, not a translation difference).
fn convect_in_scope(r: &Row, p: Precision) -> bool {
    !(p == Precision::Real4 && r.args[0] == 73.0)
}

/// `convect`: the full mass displacement matrix `FMASS(i, 1..NL+1)` for every
/// call that convects.
#[test]
fn convect_fmass_matches_flexpart() {
    check_group(
        fx(),
        "convect.fmass",
        SPREAD_ALL,
        convect_in_scope,
        eval_convect_fmass,
    );
}

/// Every port early return (`nconvtop = None`) coincides with upstream leaving
/// the sentinel in place, and vice versa — checked explicitly, so the stale
/// `NCONVTOP` is a verified property rather than an accident of the encoding.
#[test]
fn convect_nconvtop_unassigned_exactly_on_early_returns() {
    for p in [Precision::Real8, Precision::Real4] {
        let mut early = 0;
        for r in fx().rows(p).iter().filter(|r| r.function == "convect") {
            let out = run_convect(r, p);
            assert_eq!(
                out.nconvtop.is_none(),
                r.outs[6] == SENTINEL,
                "{:?}",
                r.args
            );
            early += usize::from(out.nconvtop.is_none());
        }
        println!(
            "{} convect: {early} calls return before assigning NCONVTOP",
            p.label()
        );
        assert!(early > 0);
    }
}

// ── calcmatrix (sequential replay) ────────────────────────────────────────────

/// One replayed `calcmatrix` call: its outputs in fixture order and the
/// `fmassfrac` rows.
struct CalcResult {
    outs: Vec<f64>,
    fmassfrac: HashMap<usize, Vec<f64>>,
}

fn set_calc_column(cm: &mut ConvMod, rows: &[Row], id: f64) -> ConvMetFormat {
    let c = find(rows, "calcmatrix.column", id);
    let m = NUVZ - 1;
    let fmt = format_of(c.args[1]);
    cm.psconv = c.args[2];
    for k in 1..=m {
        cm.tconv[k] = c.outs[k - 1];
        cm.qconv[k] = c.outs[m + k - 1];
        if fmt == ConvMetFormat::Gfs {
            cm.pconv[k] = c.outs[2 * m + k - 1];
        }
    }
    fmt
}

fn calc_outputs(cm: &ConvMod, lconv: bool, cb: f64) -> CalcResult {
    let m = NUVZ - 1;
    let mut o = vec![f64::from(u8::from(lconv)), cb, cm.nconvtop as f64];
    o.extend((1..=m).map(|k| cm.pconv[k]));
    o.extend((1..=NUVZ).map(|k| cm.phconv[k]));
    o.extend((1..=m).map(|k| cm.dpr[k]));
    o.extend((1..=m).map(|k| cm.qsconv[k]));
    let mut fmassfrac = HashMap::new();
    if lconv {
        for k in 1..=cm.nconvtop {
            fmassfrac.insert(k, (1..=cm.nconvtop).map(|kk| cm.fmassfrac(k, kk)).collect());
        }
    }
    CalcResult { outs: o, fmassfrac }
}

/// Replays every `calcmatrix` row in fixture order on ONE `ConvMod`, as the
/// driver ran them on one `conv_mod`. Before the first call the driver's
/// `nconvtop` holds the sentinel 777 (the last `convect` call returned early),
/// and the replay starts from that.
fn calc_replay(p: Precision) -> &'static HashMap<u64, CalcResult> {
    static R4: OnceLock<HashMap<u64, CalcResult>> = OnceLock::new();
    static R8: OnceLock<HashMap<u64, CalcResult>> = OnceLock::new();
    let cell = if p == Precision::Real4 { &R4 } else { &R8 };
    cell.get_or_init(|| {
        let rows = fx().rows(p);
        let coeffs = hybrid(rows);
        let mut cm = ConvMod::new(NUVZ, NCONVLEV);
        cm.nconvtop = SENTINEL as usize;
        let mut res = HashMap::new();
        for r in rows.iter().filter(|r| r.function == "calcmatrix") {
            let fmt = set_calc_column(&mut cm, rows, r.args[0]);
            let mut cb = r.args[2];
            let lconv = calcmatrix(&mut cm, r.args[1], &mut cb, fmt, &coeffs).lconv;
            res.insert(r.args[0] as u64, calc_outputs(&cm, lconv, cb));
        }
        res
    })
}

/// `calcmatrix`: `lconv`, the returned cloud-base mass flux, `nconvtop`
/// (including its stale value when `lconv` is false), and the column
/// diagnostics `pconv`, `phconv`, `dpr`, `qsconv`.
#[test]
fn calcmatrix_matches_flexpart() {
    check_group(
        fx(),
        "calcmatrix",
        spread_on(&CALCMATRIX_SPREAD),
        |_, _| true,
        |r, p| Some(calc_replay(p)[&(r.args[0] as u64)].outs.clone()),
    );
}

/// `calcmatrix`: the redistribution matrix `fmassfrac(k, 1..nconvtop)`.
#[test]
fn calcmatrix_fmassfrac_matches_flexpart() {
    check_group(
        fx(),
        "calcmatrix.fmassfrac",
        SPREAD_ALL,
        |_, _| true,
        |r, p| Some(calc_replay(p)[&(r.args[0] as u64)].fmassfrac[&(r.args[1] as usize)].clone()),
    );
}

/// Quirk 1 of `convmix.rs`: a stable column with a remembered cloud-base flux
/// returns `lconv = false` AND the old flux unchanged, although `convect` set
/// it to zero. Pinned on the fixture rows that show it.
#[test]
fn calcmatrix_restores_remembered_flux_when_convection_stops() {
    for p in [Precision::Real8, Precision::Real4] {
        let rows = fx().rows(p);
        let hits: Vec<&Row> = rows
            .iter()
            .filter(|r| r.function == "calcmatrix" && r.outs[0] == 0.0 && r.args[2] > 0.0)
            .collect();
        assert!(!hits.is_empty(), "no stable-with-remembered-flux row");
        for r in hits {
            assert_eq!(r.outs[1], r.args[2], "upstream restored the flux");
            assert_eq!(calc_replay(p)[&(r.args[0] as u64)].outs[1], r.args[2]);
        }
    }
}

// ── redist (sequential replay) ────────────────────────────────────────────────

fn redist_replay(p: Precision) -> &'static HashMap<u64, Vec<f64>> {
    static R4: OnceLock<HashMap<u64, Vec<f64>>> = OnceLock::new();
    static R8: OnceLock<HashMap<u64, Vec<f64>>> = OnceLock::new();
    let cell = if p == Precision::Real4 { &R4 } else { &R8 };
    cell.get_or_init(|| {
        let rows = fx().rows(p);
        let coeffs = hybrid(rows);
        let mut cm = ConvMod::new(NUVZ, NCONVLEV);
        let mut col = -1.0;
        let mut res = HashMap::new();
        for r in rows.iter().filter(|r| r.function == "redist") {
            let a = &r.args;
            if a[1] != col {
                col = a[1];
                let c = find(rows, "calcmatrix", col);
                let fmt = set_calc_column(&mut cm, rows, col);
                let mut cb = c.args[2];
                calcmatrix(&mut cm, c.args[1], &mut cb, fmt, &coeffs);
            }
            cm.tt2conv = a[6];
            cm.td2conv = a[7];
            let mut ktop = a[2] as i32;
            let mut draws = std::iter::once(a[9]);
            let out = redist(
                &mut cm,
                a[8],
                &mut ktop,
                a[3] as i32,
                a[4] as i64,
                a[5],
                &mut draws,
            );
            res.insert(
                a[0] as u64,
                vec![
                    out.z,
                    f64::from(out.ipconv),
                    f64::from(ktop),
                    f64::from(u8::from(out.drew)),
                ],
            );
        }
        res
    })
}

/// `redist`: new height, `ipconv`, `ktop` and whether a draw was taken.
#[test]
fn redist_matches_flexpart() {
    check_group(
        fx(),
        "redist",
        spread_on(&REDIST_SPREAD),
        |_, _| true,
        |r, p| Some(redist_replay(p)[&(r.args[0] as u64)].clone()),
    );
}

// ── sort2 ─────────────────────────────────────────────────────────────────────

/// The port's stable sort against upstream's `sort2` (Numerical Recipes
/// quicksort, called by the driver, not ported). The sorted keys must be
/// identical; the permutation must be identical whenever it is determined
/// (no tied keys) or `sort2` runs only its insertion sort (`n <= 7`), and the
/// remaining tie-order differences are counted and printed.
#[test]
fn sort2_permutation_vs_stable_sort() {
    for p in [Precision::Real8, Precision::Real4] {
        for r in fx().rows(p).iter().filter(|r| r.function == "sort2") {
            let n = r.args[0] as usize;
            let keys: Vec<i64> = r.args[1..=n].iter().map(|&k| k as i64).collect();
            let up_sorted: Vec<i64> = r.outs[0..n].iter().map(|&k| k as i64).collect();
            let up_perm: Vec<usize> = r.outs[n..2 * n].iter().map(|&i| i as usize - 1).collect();
            let (sorted, perm) = sort_particles_by_column(&keys);
            assert_eq!(sorted, up_sorted, "sorted keys, n = {n}");
            let mut distinct = keys.clone();
            distinct.sort_unstable();
            distinct.dedup();
            let ties = distinct.len() < n;
            let differ = perm.iter().zip(&up_perm).filter(|(a, b)| a != b).count();
            if p == Precision::Real8 {
                println!("sort2 n = {n:3}: tied keys {ties:5}, positions where the permutation differs: {differ}");
            }
            if !ties || n <= 7 {
                assert_eq!(perm, up_perm, "n = {n}");
            }
        }
    }
}

// ── convmix (sequential replay, draws matched per particle) ──────────────────

const MX: usize = 5;
const MY: usize = 4;
const NNX: usize = 5;
const NNY: usize = 5;

fn convmix_grids(rows: &[Row]) -> (ConvMixMet, ConvMixNest) {
    let blank = |nx: usize, ny: usize| ConvMixMet {
        nx,
        ny,
        nlev: NUVZ,
        ps: [vec![0.0; nx * ny], vec![0.0; nx * ny]],
        tt2: [vec![0.0; nx * ny], vec![0.0; nx * ny]],
        td2: [vec![0.0; nx * ny], vec![0.0; nx * ny]],
        t3: [vec![0.0; nx * ny * NUVZ], vec![0.0; nx * ny * NUVZ]],
        q3: [vec![0.0; nx * ny * NUVZ], vec![0.0; nx * ny * NUVZ]],
        p3: [vec![0.0; nx * ny * NUVZ], vec![0.0; nx * ny * NUVZ]],
        cbaseflux: vec![0.0; nx * ny],
    };
    let mut mother = blank(MX, MY);
    let mut nest = blank(NNX, NNY);
    for r in rows.iter().filter(|r| r.function == "convmix.met") {
        let met = if r.args[0] == 0.0 {
            &mut mother
        } else {
            &mut nest
        };
        let (m, ix, jy) = (
            r.args[1] as usize - 1,
            r.args[2] as usize,
            r.args[3] as usize,
        );
        let c2 = ix + met.nx * jy;
        met.ps[m][c2] = r.outs[0];
        met.tt2[m][c2] = r.outs[1];
        met.td2[m][c2] = r.outs[2];
        for k in 0..NUVZ {
            met.t3[m][c2 * NUVZ + k] = r.outs[3 + k];
            met.q3[m][c2 * NUVZ + k] = r.outs[20 + k];
            met.p3[m][c2 * NUVZ + k] = r.outs[37 + k];
        }
    }
    let nest = ConvMixNest {
        met: nest,
        xln: 1.0,
        yln: 0.5,
        xrn: 3.0,
        yrn: 2.5,
        xresoln: 2.0,
        yresoln: 2.0,
    };
    (mother, nest)
}

/// Per call: the port's outputs in fixture order, the number of within-column
/// permutation differences from `sort2`, and the number of particles moved.
struct MixResult {
    outs: Vec<f64>,
    perm_differences: usize,
}

fn convmix_replay(p: Precision) -> &'static HashMap<u64, MixResult> {
    static R4: OnceLock<HashMap<u64, MixResult>> = OnceLock::new();
    static R8: OnceLock<HashMap<u64, MixResult>> = OnceLock::new();
    let cell = if p == Precision::Real4 { &R4 } else { &R8 };
    cell.get_or_init(|| {
        let rows = fx().rows(p);
        let coeffs = hybrid(rows);
        let (mut mother, nest) = convmix_grids(rows);
        let mut nests = vec![nest];
        let mut cm = ConvMod::new(NUVZ, NCONVLEV);
        let mut res = HashMap::new();
        for r in rows.iter().filter(|r| r.function == "convmix") {
            let ic = r.args[0];
            let a = &r.args;
            let prm = ConvMixParams {
                itime: a[1] as i64,
                memtime: [a[2] as i64, a[3] as i64],
                memind: [a[4] as usize, a[5] as usize],
                lsynctime: a[6] as i64,
                ldirect: a[7] as i32,
                format: format_of(a[8]),
                height_nz: a[9],
                nxmax: 361,
                coeffs: coeffs.clone(),
            };
            let prows: Vec<&Row> = rows
                .iter()
                .filter(|q| q.function == "convmix.particle" && q.args[0] == ic)
                .collect();
            let mut parts = ConvParticles {
                itra1: prows.iter().map(|q| q.outs[0] as i64).collect(),
                xtra1: prows.iter().map(|q| q.outs[1]).collect(),
                ytra1: prows.iter().map(|q| q.outs[2]).collect(),
                ztra1: prows.iter().map(|q| q.outs[3]).collect(),
            };
            let np = parts.ztra1.len();
            let queue = &rows
                .iter()
                .find(|q| q.function == "convmix.draws" && q.args[0] == ic)
                .expect("draws row")
                .outs;

            // The port's keys must be the keys upstream sorted.
            let (igrid, igridn) = convmix_column_keys(&mother, &nests, &parts, &prm);
            let mut upstream_order = Vec::new();
            let mut perm_differences = 0;
            for (g, keys) in [igrid, igridn[0].clone()].iter().enumerate() {
                let o = &rows
                    .iter()
                    .find(|q| {
                        q.function == "convmix.order" && q.args[0] == ic && q.args[1] == g as f64
                    })
                    .expect("order row")
                    .outs;
                let up_perm: Vec<usize> = o[0..np].iter().map(|&i| i as usize - 1).collect();
                let up_keys: Vec<i64> = o[np..2 * np].iter().map(|&k| k as i64).collect();
                let (sorted, perm) = sort_particles_by_column(keys);
                assert_eq!(
                    sorted,
                    up_keys,
                    "{} convmix call {ic} grid {g}: column keys",
                    p.label()
                );
                perm_differences += perm.iter().zip(&up_perm).filter(|(a, b)| a != b).count();
                for (kp, &ip) in up_perm.iter().enumerate() {
                    if up_keys[kp] != -1 {
                        upstream_order.push((ip, g));
                    }
                }
            }

            // Pass 1 (on a copy): which particles draw. Independent of the
            // draw values: it depends only on height and column.
            let mut probe = (cm.clone(), mother.clone(), nests.clone(), parts.clone());
            let rep = convmix(
                &mut probe.0,
                &mut probe.1,
                &mut probe.2,
                &mut probe.3,
                &prm,
                &mut std::iter::repeat(0.5),
            );
            let drew: std::collections::HashSet<(usize, usize)> = rep
                .visited
                .iter()
                .filter(|v| v.2)
                .map(|v| (v.0, v.1))
                .collect();
            // Upstream gives queue[n] to the n-th drawing particle in ITS order.
            let mut draw_of = HashMap::new();
            let mut n = 0;
            for key in upstream_order.iter().filter(|k| drew.contains(k)) {
                draw_of.insert(*key, queue[n]);
                n += 1;
            }
            let port_draws: Vec<f64> = rep
                .visited
                .iter()
                .filter(|v| v.2)
                .map(|v| draw_of[&(v.0, v.1)])
                .collect();

            // Pass 2 on the real state.
            let rep2 = convmix(
                &mut cm,
                &mut mother,
                &mut nests,
                &mut parts,
                &prm,
                &mut port_draws.into_iter(),
            );
            assert_eq!(rep2.draws_used, n);
            let mut o = parts.ztra1.clone();
            o.push(rep2.draws_used as f64);
            o.extend(&mother.cbaseflux);
            o.extend(&nests[0].met.cbaseflux);
            res.insert(
                ic as u64,
                MixResult {
                    outs: o,
                    perm_differences,
                },
            );
        }
        res
    })
}

/// `convmix`: every particle's new height, the draws consumed, and the
/// mother and nest `cbaseflux` after each of four successive calls.
#[test]
fn convmix_matches_flexpart() {
    check_group(
        fx(),
        "convmix",
        spread_on(&CONVMIX_SPREAD),
        |_, _| true,
        |r, p| Some(convmix_replay(p)[&(r.args[0] as u64)].outs.clone()),
    );
    for ic in 1..=4 {
        println!(
            "convmix call {ic}: positions where the stable sort's particle order differs from sort2's: {}",
            convmix_replay(Precision::Real8)[&ic].perm_differences
        );
    }
}
