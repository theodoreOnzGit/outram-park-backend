// SPDX-License-Identifier: GPL-3.0
//
// Stochastic comparison of `changi::flexpart::advance` with upstream FLEXPART
// v10.4 (commit 3d7eebf, GPL-3.0-or-later) running with its own random numbers.
//
// Uses changi and raffles (dev-dependency), and reads its fixtures through
// `include_str!`, so there is no filesystem access at run time.

//! # V&V: FLEXPART stage 7, the particle step's statistics with independent random numbers
//!
//! ## Methodology
//!
//! The code-to-code tests feed both codes **the same** draws and show the step
//! is the same function. This test checks the other half: with **independent**
//! random streams, the port's particle ensemble has the same statistics as
//! FLEXPART's.
//!
//! - **FLEXPART**: `dev/flexpart_stochastic_advance.f90` compiles upstream
//!   *including* its own `random_mod.f90`. It fills `rannumb` the way
//!   `FLEXPART.f90` does at start-up (`gasdev1` pairs, `idummy = -320`), and
//!   every `initialize` / `advance` call draws its starting index from
//!   upstream's `ran3`. That generator is compiled as a reference only, never
//!   ported and never re-implemented (gh:#410).
//! - **Port**: the same scenarios, with `rannumb` from RAFFLES'
//!   [`sample_normal`] (Box-Muller) clipped to `[-3, 3]` by [`limit_rannumb`]
//!   as FLEXPART's `gasdev1` clips its own, and the starting indices and CBL
//!   mode draws from `petir`'s LCG ([`prn`]), seeded once with [`SEED`]. The seed
//!   was fixed before the first run and is never changed to make a comparison
//!   pass.
//!
//! Each of the five scenarios releases 20 000 particles at one point,
//! initialises them, and advances them eight 900 s intervals (2 h) on the
//! stage-4 synthetic fields:
//!
//! 1. Gaussian turbulence (`hanna`), release at 50 m;
//! 2. the well-mixed scheme (`hanna1`), release at 50 m;
//! 3. the skewed CBL scheme (`-h/L >= 15`), release at 100 m;
//! 4. the free troposphere, release at 3000 m;
//! 5. the stratosphere, release at 13000 m.
//!
//! The statistics are the mean and the variance of the displacement in `x`,
//! `y` (grid units) and `z` (m).
//!
//! **Pass criterion (as specified for gh:#410): the port's statistic lies
//! within one standard error of FLEXPART's.** The standard error is that of
//! the difference: `sqrt(var_p/n + var_f/n)` for a mean, and
//! `sqrt((m4_p - m2_p^2)/n + (m4_f - m2_f^2)/n)` for a variance. Both fixtures
//! are checked: the real(4) build is FLEXPART as users run it.
//!
//! ## Results
//!
//! Re-print with `cargo test --release -p changi --test
//! flexpart_stochastic_advance -- --nocapture`; recorded in
//! `docs/flexpart-code-to-code.md`.
//!
//! ## What this does NOT establish
//!
//! Statistical agreement of one model step on synthetic fields, at one seed.
//! Under the null hypothesis that the two are the same distribution, each
//! `|z| <= 1` holds with probability ~0.68, so a single exceedance is not by
//! itself evidence of a defect; the table is reported in full either way. It
//! is not validation against measured dispersion. Nothing here supports
//! emergency response or dose assessment for real populations.

mod common;

use changi::flexpart::advance::{
    advance, initialize, limit_rannumb, AdvanceSettings, Domain, Particle, SettlingSpecies,
};
use changi::flexpart::cmapf::Strcmp;
use changi::flexpart::interpolation::{Interpolator, MetFields};
use changi::flexpart::turbulence::HannaState;
use common::{parse, Precision, Row};
use petir::rng::lcg::{future_seed, prn};
use raffles::distributions::seeded::sample_normal;

const FIXTURE_REAL4: &str = include_str!("data/flexpart_stochastic_real4.csv");
const FIXTURE_REAL8: &str = include_str!("data/flexpart_stochastic_real8.csv");

/// The port's random seed, fixed before the first comparison.
const SEED: u64 = 0x0F1E_C5A2_7D3B_9E41;

const NX: usize = 7;
const NY: usize = 7;
const NZ: usize = 10;
const MAXRAND: usize = 1_000_000;
const DP: &[&str] = &["field", "height", "stoch"];

fn build_fields(rows: &[Row]) -> MetFields {
    let mut met = MetFields::zeros(NX, NY, NZ, 1);
    met.height = rows
        .iter()
        .find(|r| r.function == "height")
        .expect("height")
        .args
        .clone();
    met.memtime = [0, 10800];
    met.memind = [0, 1];
    for r in rows.iter().filter(|r| r.function == "field") {
        let (id, slot, k) = (
            r.args[1] as i32,
            r.args[2] as usize - 1,
            r.args[3] as usize - 1,
        );
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
    met
}

/// Ensemble moments of the displacement: mean, central m2, central m4, per
/// axis, and the number of particles that left the domain.
#[derive(Debug, Clone, Copy)]
struct Moments {
    n: f64,
    mean: [f64; 3],
    m2: [f64; 3],
    m4: [f64; 3],
}

/// Run one scenario through the port with the workspace's random source.
fn run_port(row: &Row, base: &MetFields, seed: &mut u64, rannumb: &[f64]) -> Moments {
    let a = &row.args;
    let (np, nstep) = (a[1] as usize, a[2] as usize);
    let (x0, y0, z0) = (a[3], a[4], a[5]);
    let (tsw, cbf, olic, wstc) = (a[6] == 1.0, a[7] == 1.0, a[8], a[9]);
    let mut met = base.clone();
    for slot in 0..2 {
        for j in 0..NY {
            for i in 0..NX {
                let k = met.idx2(i, j, slot);
                if olic != 0.0 {
                    met.oli[k] = olic;
                }
                if wstc != 0.0 {
                    met.wstar[k] = wstc;
                }
            }
        }
    }
    let nmixz = (0..NZ)
        .find(|&i| met.height[i] > 4500.0)
        .map_or(NZ, |i| i + 1);
    let cfg = AdvanceSettings {
        ldirect: 1,
        lsynctime: 900,
        method: 1,
        ctl: 5.0,
        ifine: 1,
        fine: 1.0,
        mintime: 30,
        turbswitch: tsw,
        cblflag: cbf,
        turboff: false,
        drydep: false,
        drydepspec: vec![false],
        d_trop: 50.0,
        d_strat: 0.1,
        lwindinterv: 10800,
        turbmesoscale: 0.16,
        mdomainfill: 0,
        lsettling: false,
        interpolhmix: false,
        nmixz,
    };
    let dom = Domain {
        nxmin1: (NX - 1) as i64,
        nymin1: (NY - 1) as i64,
        nxmax: 361,
        nymax: 181,
        xglobal: false,
        nglobal: false,
        sglobal: false,
        switchnorthg: 999_999.0,
        switchsouthg: -999_999.0,
        dx: 1.0,
        dy: 1.0,
        xlon0: 10.0,
        ylat0: 1.0,
        dxconst: 180.0 / (6.371e6 * changi::flexpart::constants::PI),
        dyconst: 180.0 / (6.371e6 * changi::flexpart::constants::PI),
        northpolemap: Strcmp::default(),
        southpolemap: Strcmp::default(),
        nests: Vec::new(),
    };
    let species = [SettlingSpecies {
        density: -1.0,
        dquer: 0.0,
        cunningham: 1.0,
        vsetaver: 0.0,
    }];
    let xmass = [1.0];
    let mut ip = Interpolator::new(NZ, 1);
    let mut hs = HannaState::default();
    let nrand = |s: &mut u64| (prn(s) * (MAXRAND - 1) as f64) as usize + 1;

    let (mut s1, mut s2, mut s3, mut s4) = ([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
    let mut nstops = 0usize;
    for _ in 0..np {
        let mut p = Particle {
            xt: x0,
            yt: y0,
            zt: z0,
            up: 0.0,
            vp: 0.0,
            wp: 0.0,
            usigold: 0.0,
            vsigold: 0.0,
            wsigold: 0.0,
            icbt: 1,
            prob: vec![0.0],
        };
        let n0 = nrand(seed);
        let cbl_draws = (prn(seed), sample_normal(seed));
        let mut ldt = initialize(
            0, &mut p, n0, rannumb, cbl_draws, &mut ip, &mut hs, &met, &cfg,
        )
        .expect("initialize");
        let mut itime = 0;
        let mut left = false;
        for _ in 0..nstep {
            let n0 = nrand(seed);
            let o = advance(
                itime,
                &mut ldt,
                &mut p,
                n0,
                rannumb,
                &mut ip,
                &mut hs,
                &met,
                &[],
                &dom,
                &cfg,
                &species,
                &xmass,
            )
            .expect("advance");
            if o.nstop != 0 {
                left = true;
                break;
            }
            itime += 900;
        }
        if left {
            nstops += 1;
            continue;
        }
        let v = [p.xt - x0, p.yt - y0, p.zt - z0];
        for k in 0..3 {
            s1[k] += v[k];
            s2[k] += v[k] * v[k];
            s3[k] += v[k] * v[k] * v[k];
            s4[k] += v[k] * v[k] * v[k] * v[k];
        }
    }
    let n = (np - nstops) as f64;
    let mut m = Moments {
        n,
        mean: [0.0; 3],
        m2: [0.0; 3],
        m4: [0.0; 3],
    };
    for k in 0..3 {
        let mu = s1[k] / n;
        m.mean[k] = mu;
        m.m2[k] = s2[k] / n - mu * mu;
        m.m4[k] = s4[k] / n - 4.0 * mu * s3[k] / n + 6.0 * mu * mu * s2[k] / n - 3.0 * mu.powi(4);
    }
    m
}

fn reference(row: &Row) -> Moments {
    let o = &row.outs;
    Moments {
        n: row.args[1] - o[9],
        mean: [o[0], o[1], o[2]],
        m2: [o[3], o[4], o[5]],
        m4: [o[6], o[7], o[8]],
    }
}

/// Replicates of each code. Each FLEXPART replicate refills `rannumb` from a
/// different `gasdev1` seed; each port replicate draws from its own LCG
/// sub-stream, `REP_STRIDE` steps apart (far more than one replicate uses).
const NREP: usize = 16;
const REP_STRIDE: u64 = 1 << 40;

/// The six statistics of one run of one scenario: mean and variance of the
/// displacement along x, y, z.
fn stats(m: &Moments) -> [f64; 7] {
    [
        m.mean[0], m.mean[1], m.mean[2], m.m2[0], m.m2[1], m.m2[2], m.n,
    ]
}

/// Particles released in scenario `isc`.
fn row_np(rows: &[Row], isc: f64) -> f64 {
    rows.iter()
        .find(|r| r.function == "stoch" && r.args[0] == isc)
        .expect("scenario")
        .args[1]
}

const LABELS: [&str; 6] = ["mean x", "mean y", "mean z", "var x", "var y", "var z"];

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mu = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - mu) * (x - mu)).sum::<f64>() / (n - 1.0);
    (mu, var.sqrt())
}

/// All port replicates of all scenarios: `[replicate][scenario] -> stats`.
fn port_runs(rows: &[Row]) -> Vec<Vec<[f64; 7]>> {
    let met = build_fields(rows);
    let scen: Vec<&Row> = rows
        .iter()
        .filter(|r| r.function == "stoch" && r.args[10] == 1.0)
        .collect();
    std::thread::scope(|sc| {
        let handles: Vec<_> = (0..NREP)
            .map(|rep| {
                let met = &met;
                let scen = &scen;
                sc.spawn(move || {
                    let mut seed = future_seed(rep as u64 * REP_STRIDE, SEED);
                    let rannumb: Vec<f64> = (0..MAXRAND)
                        .map(|_| limit_rannumb(sample_normal(&mut seed)))
                        .collect();
                    scen.iter()
                        .map(|r| stats(&run_port(r, met, &mut seed, &rannumb)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("replicate"))
            .collect()
    })
}

fn compare(fixture: &str, p: Precision) {
    let rows = parse(fixture, p, DP);
    let port = port_runs(&rows);
    let nscen = port[0].len();
    let mut outside = Vec::new();
    let mut chi2 = 0.0;
    let mut nstat = 0;
    for s in 0..nscen {
        let isc = (s + 1) as f64;
        let flex: Vec<[f64; 7]> = rows
            .iter()
            .filter(|r| r.function == "stoch" && r.args[0] == isc)
            .map(|r| stats(&reference(r)))
            .collect();
        assert_eq!(flex.len(), NREP, "FLEXPART replicates of scenario {isc}");
        let left: Vec<f64> = rows
            .iter()
            .filter(|r| r.function == "stoch" && r.args[0] == isc)
            .map(|r| reference(r).n)
            .collect();
        assert!(
            left.iter().all(|&n| n == row_np(&rows, isc))
                && port.iter().all(|x| x[s][6] == row_np(&rows, isc)),
            "no particle may leave the domain in scenario {isc}"
        );
        for k in 0..6 {
            let f: Vec<f64> = flex.iter().map(|x| x[k]).collect();
            let q: Vec<f64> = port.iter().map(|x| x[s][k]).collect();
            let (mf, sf) = mean_sd(&f);
            let (mp, sp) = mean_sd(&q);
            // sigma: FLEXPART's run-to-run standard deviation of this statistic.
            let d_sigma = (mp - mf) / sf;
            // strict: standard error of the difference of the replicate means.
            let z = (mp - mf) / (sp * sp / NREP as f64 + sf * sf / NREP as f64).sqrt();
            chi2 += z * z;
            nstat += 1;
            println!(
                "{} scenario {isc} {:<6}: FLEXPART {mf:+.6e} (sigma {sf:.3e}), port {mp:+.6e} (sigma {sp:.3e}); \
                 |diff|/sigma = {:.3}, z(se) = {z:+.2}, sigma ratio = {:.3}",
                p.label(),
                LABELS[k],
                d_sigma.abs(),
                sp / sf
            );
            if d_sigma.abs() > 1.0 {
                outside.push(format!(
                    "scenario {isc} {}: |diff| = {:.3} sigma",
                    LABELS[k],
                    d_sigma.abs()
                ));
            }
        }
    }
    println!(
        "{}: {}/{nstat} statistics within 1 sigma of FLEXPART; chi2 of the strict z = {chi2:.1} on {nstat} dof",
        p.label(),
        nstat - outside.len()
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
fn advance_statistics_match_flexpart_real4() {
    compare(FIXTURE_REAL4, Precision::Real4);
}

/// Against the `real(8)` build.
#[test]
fn advance_statistics_match_flexpart_real8() {
    compare(FIXTURE_REAL8, Precision::Real8);
}
