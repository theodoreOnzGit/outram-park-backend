// SPDX-License-Identifier: GPL-3.0
//
// Stochastic comparison of `changi::flexpart::convmix::redist` with upstream
// FLEXPART v10.4 (commit 3d7eebf, GPL-3.0-or-later) drawing from its own ran3.

//! # V&V: FLEXPART stage 7b, convective redistribution with independent random numbers
//!
//! ## Methodology
//!
//! The code-to-code test (`flexpart_convection_code_to_code`) feeds `redist`
//! and the port the same uniform numbers. This test checks the statistics
//! with **independent** streams.
//!
//! **FLEXPART.** `dev/flexpart_stochastic_redist.f90` compiles upstream,
//! including `random_mod.f90`, so `redist.f90` draws from upstream's `ran3`
//! (its saved `iseed = -88`, one continuing stream).
//!
//! **The column.** One convecting ECMWF column: Ts = 308 K, RH 0.92/0.75.
//! `calcmatrix` is run for 200 steps of 900 s, so the cloud-base flux builds up
//! as `convmix` builds it. The column and `calcmatrix`'s result are echoed. The
//! port rebuilds the matrix with its own `calcmatrix` and asserts it is the
//! same.
//!
//! **The sample.** Each replicate redistributes 20 000 particles once, starting
//! on a fixed height grid from 0 to 14 km. FLEXPART runs 16 replicates
//! (consecutive blocks of its `ran3` stream). The port runs 16 replicates with
//! uniforms from `petir`'s LCG ([`prn`]), on sub-streams `2^40` apart from a
//! seed fixed before the first run.
//!
//! **The statistics.** The mean and variance of the final height for starts in
//! [0, 1.5), [1.5, 6) and [6, 14) km, and the fraction of particles the matrix
//! moved.
//!
//! **The criterion** (gh:#410). The port's replicate mean must lie within 1σ
//! of FLEXPART's, where σ is FLEXPART's run-to-run standard deviation of that
//! statistic. The strict z of the difference of the two replicate means, and
//! its χ², are reported too.
//!
//! ## Results
//!
//! Re-print with `cargo test --release -p changi --test
//! flexpart_stochastic_redist -- --nocapture`; recorded in
//! `docs/flexpart-code-to-code.md`.
//!
//! ## What this does NOT establish
//!
//! Statistical agreement of one redistribution step on one synthetic column.
//! It is not validation. Nothing here supports emergency response or dose
//! assessment for real populations.

mod common;

use changi::flexpart::convmix::{calcmatrix, redist, ConvMetFormat, ConvMod, HybridCoefficients};
use common::{parse, Precision, Row};
use petir::rng::lcg::{future_seed, prn};

const FIXTURE_REAL4: &str = include_str!("data/flexpart_stochastic_redist_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_stochastic_redist_real8.csv");

/// The port's random seed, fixed before the first comparison.
const SEED: u64 = 0x6C1D_0A3E_95F2_4B87;
const NREP: usize = 16;
const REP_STRIDE: u64 = 1 << 40;
const NUVZ: usize = 17;
const NCONVLEV: usize = 15;
const LABELS: [&str; 7] = [
    "mean z [0,1.5)",
    "var z [0,1.5)",
    "mean z [1.5,6)",
    "var z [1.5,6)",
    "mean z [6,14)",
    "var z [6,14)",
    "moved",
];

fn find<'a>(rows: &'a [Row], name: &str) -> &'a Row {
    rows.iter()
        .find(|r| r.function == name)
        .unwrap_or_else(|| panic!("no `{name}` row"))
}

fn hybrid(rows: &[Row]) -> HybridCoefficients {
    let r = find(rows, "hybrid");
    let n = r.args[0] as usize;
    let o = &r.outs;
    HybridCoefficients {
        akz: o[0..n].to_vec(),
        bkz: o[n..2 * n].to_vec(),
        akm: o[2 * n..3 * n].to_vec(),
        bkm: o[3 * n..4 * n].to_vec(),
    }
}

/// The column, spun up with the port's `calcmatrix`, checked against the
/// echoed upstream result.
fn column(rows: &[Row], p: Precision) -> ConvMod {
    let coeffs = hybrid(rows);
    let mut cm = ConvMod::new(NUVZ, NCONVLEV);
    let c = find(rows, "redist.column");
    let m = NUVZ - 1;
    cm.psconv = c.args[2];
    for k in 1..=m {
        cm.tconv[k] = c.outs[k - 1];
        cm.qconv[k] = c.outs[m + k - 1];
    }
    let spin = find(rows, "redist.calcmatrix");
    let nspin = spin.outs[0] as usize;
    let mut cb = 0.0;
    let mut lconv = false;
    for _ in 0..nspin {
        lconv = calcmatrix(
            &mut cm,
            p.lit(900.0),
            &mut cb,
            ConvMetFormat::Ecmwf,
            &coeffs,
        )
        .lconv;
    }
    assert!(lconv, "the column must convect");
    assert_eq!(cm.nconvtop as f64, spin.args[2], "cloud top");
    let rel = (cb - spin.args[1]).abs() / spin.args[1];
    // Real(8): the spin-up is bit-exact. Real(4): 200 chained steps of the
    // DTMA-limited flux (see the convection test) agree to its precision.
    let tol = if p == Precision::Real8 { 0.0 } else { 1e-3 };
    assert!(
        rel <= tol,
        "{}: cloud-base flux {cb:e} vs FLEXPART {:e}",
        p.label(),
        spin.args[1]
    );
    let s = find(rows, "redist.surface");
    cm.tt2conv = s.args[0];
    cm.td2conv = s.args[1];
    cm
}

/// The seven statistics of one port replicate.
fn port_replicate(cm0: &ConvMod, height_nz: f64, np: usize, rep: usize) -> [f64; 7] {
    let mut cm = cm0.clone();
    let mut seed = future_seed(rep as u64 * REP_STRIDE, SEED);
    let mut draws = std::iter::from_fn(move || Some(prn(&mut seed)));
    let (mut s1, mut s2, mut nb) = ([0.0; 3], [0.0; 3], [0usize; 3]);
    let mut moved = 0usize;
    let mut ktop = 0;
    for i in 1..=np {
        // The driver's heights: double precision, rounded once to real(4).
        let z0 = ((i as f64 - 0.5) * 14000.0 / np as f64) as f32 as f64;
        let r = redist(&mut cm, z0, &mut ktop, 1, 900, height_nz, &mut draws);
        let b = if z0 < 1500.0 {
            0
        } else if z0 < 6000.0 {
            1
        } else {
            2
        };
        nb[b] += 1;
        s1[b] += r.z;
        s2[b] += r.z * r.z;
        if r.ipconv == -1 {
            moved += 1;
        }
    }
    let mut out = [0.0; 7];
    for b in 0..3 {
        let mu = s1[b] / nb[b] as f64;
        out[2 * b] = mu;
        out[2 * b + 1] = s2[b] / nb[b] as f64 - mu * mu;
    }
    out[6] = moved as f64 / np as f64;
    out
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mu = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - mu) * (x - mu)).sum::<f64>() / (n - 1.0);
    (mu, var.sqrt())
}

fn compare(fixture: &str, p: Precision) {
    let rows = parse(fixture, p, &[]);
    let cm = column(&rows, p);
    let height_nz = find(&rows, "redist.surface").args[2];
    let flex: Vec<&Row> = rows
        .iter()
        .filter(|r| r.function == "redist.stats")
        .collect();
    assert_eq!(flex.len(), NREP);
    let np = flex[0].args[1] as usize;
    let port: Vec<[f64; 7]> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..NREP)
            .map(|rep| {
                let cm = &cm;
                sc.spawn(move || port_replicate(cm, height_nz, np, rep))
            })
            .collect();
        hs.into_iter()
            .map(|h| h.join().expect("replicate"))
            .collect()
    });
    let mut outside = Vec::new();
    let mut chi2 = 0.0;
    for k in 0..7 {
        let f: Vec<f64> = flex.iter().map(|r| r.outs[k]).collect();
        let q: Vec<f64> = port.iter().map(|x| x[k]).collect();
        let (mf, sf) = mean_sd(&f);
        let (mp, sp) = mean_sd(&q);
        let d_sigma = (mp - mf) / sf;
        let z = (mp - mf) / (sp * sp / NREP as f64 + sf * sf / NREP as f64).sqrt();
        chi2 += z * z;
        println!(
            "{} {:<15}: FLEXPART {mf:+.6e} (sigma {sf:.3e}), port {mp:+.6e} (sigma {sp:.3e}); \
             |diff|/sigma = {:.3}, z(se) = {z:+.2}, sigma ratio = {:.3}",
            p.label(),
            LABELS[k],
            d_sigma.abs(),
            sp / sf
        );
        if d_sigma.abs() > 1.0 {
            outside.push(format!(
                "{}: |diff| = {:.3} sigma",
                LABELS[k],
                d_sigma.abs()
            ));
        }
    }
    println!(
        "{}: {}/7 statistics within 1 sigma of FLEXPART; chi2 of the strict z = {chi2:.1} on 7 dof",
        p.label(),
        7 - outside.len()
    );
    assert!(
        outside.is_empty(),
        "{}: outside 1 sigma:\n{}",
        p.label(),
        outside.join("\n")
    );
}

/// Against FLEXPART as shipped (`real(4)`).
#[test]
fn redist_statistics_match_flexpart_real4() {
    compare(FIXTURE_REAL4, Precision::Real4);
}

/// Against the `real(8)` build.
#[test]
fn redist_statistics_match_flexpart_real8() {
    compare(FIXTURE_REAL8, Precision::Real8);
}
