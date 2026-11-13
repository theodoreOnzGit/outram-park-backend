//! `--generate-artefacts`: the natively precomputed half of the twin
//! (module docs of [`crate::artefacts`]). **No new model, no new setting**:
//! it calls the twin engine's double-pipe code exactly as gh:#1180's and
//! gh:#1190's tests do, then evaluates the results on [`artefacts::grid`].
//!
//! - **KOH:** `KohCalibration::calibrate(TuasOneDUa, ua_dataset(44 rows),
//!   design = the 44 x, koh_spec(WithDiscrepancy))`, the call in
//!   `double_pipe::koh`'s `koh_calibration_of_the_double_pipe`; predictive
//!   at stride 5 with the observation error (as that test's held-out
//!   check), in chunks of [`CHUNK`] points (the predictor is joint over
//!   the points it is given; only each point's marginal is kept).
//! - **MFK:** `Surrogate::fit_multi(CoKriging(MfkConfig::smt_defaults()))`
//!   on the committed `lf_grid` (375 TUAS rows) and the 36 CFD training rows,
//!   **UA only** (gh:#1190 fitted the nine outputs one MFK each; UA alone is
//!   that UA fit, which the held-out RMSE below checks).
//! - **1-D:** `one_d::ua` at nominal θ and at the posterior-mean θ
//!   (`ThetaEstimate::point`), the numbers the browser recomputes live.
//!
//! It prints the held-out check (gh:#1180: KOH UA RMSE 1.28 %, 95 % coverage
//! 8/8, 1-D 8.95 % / 6.30 %; gh:#1190: MFK UA 3.0 %) so a regenerated
//! artefact is compared with the records it claims to reproduce.

use crate::artefacts::{self, GridRow, GRID_HEADER, MEAN_TAG};
use outram_park_digital_twin_engine::multifidelity::double_pipe::design::Split as DpSplit;
use outram_park_digital_twin_engine::multifidelity::double_pipe::fidelity::ONE_D_RANK;
use outram_park_digital_twin_engine::multifidelity::double_pipe::koh::{
    koh_spec, ua_dataset, TuasOneDUa, UaOutput,
};
use outram_park_digital_twin_engine::multifidelity::double_pipe::{
    cokriging, dataset, lf_grid, one_d, DoublePipeInput, Theta,
};
use raffles::bayesian::koh::KohForm;
use raffles::fidelity::koh::KohCalibration;
use raffles::fidelity::{
    Dataset, Fidelity, MultiFidelityDataset, Quantities, Split, Surrogate, SurrogateSpec,
};
use raffles::surrogate::MfkConfig;
use std::fmt::Write as _;
use uom::si::thermal_conductance::watt_per_kelvin;

/// Points per KOH predictor call.
pub const CHUNK: usize = 45;
/// Posterior samples per predictive mixture (gh:#1180's held-out stride).
pub const STRIDE: usize = 5;
/// Every this many posterior samples is written to the θ artefact.
pub const THIN: usize = 10;

/// UA of `x` at `theta` in W/K.
fn ua(x: &DoublePipeInput, theta: &Theta) -> Result<f64, String> {
    one_d::ua(x, theta)
        .map(|u| u.get::<watt_per_kelvin>())
        .map_err(|e| e.to_string())
}

/// The UA-only co-kriging dataset: `lf_grid` (all training) and the CFD
/// rows with their stored split.
///
/// # Errors
/// The committed CSVs or raffles' constructors.
pub fn mfk_dataset() -> Result<MultiFidelityDataset<DoublePipeInput, UaOutput>, String> {
    let rows = lf_grid::load_committed()?;
    let x: Vec<DoublePipeInput> = rows.iter().map(|r| r.0).collect();
    let y: Vec<UaOutput> = rows.iter().map(|r| UaOutput { ua: r.1.ua }).collect();
    let lf = Dataset::new(
        &x,
        &y,
        &vec![Split::Training; x.len()],
        Fidelity::new(ONE_D_RANK, "TUAS 1-D, nominal theta, UA"),
        cokriging::LF_PROVENANCE,
    )
    .map_err(|e| e.to_string())?;
    let hf = ua_dataset(&dataset::load_committed()).map_err(|e| e.to_string())?;
    MultiFidelityDataset::new(vec![lf, hf]).map_err(|e| e.to_string())
}

/// Relative RMSE of `pred` against `truth` (over the mean |truth|), as the
/// gh:#1180 and gh:#1190 records define it.
#[must_use]
pub fn rel_rmse(pred: &[f64], truth: &[f64]) -> f64 {
    let n = truth.len() as f64;
    (pred
        .iter()
        .zip(truth)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        / n)
        .sqrt()
        / (truth.iter().map(|t| t.abs()).sum::<f64>() / n)
}

/// Run everything and write the two artefacts into `out_dir`; returns the
/// log it printed. ~20 min. CLI glue around the twin engine's calls: the
/// cheap parts it relies on (the grid, [`mfk_dataset`], [`rel_rmse`], the
/// writers' formats) are reached by tests; the 20-minute run itself is the
/// gh:#1180 test's call, not repeated in a test here.
///
/// # Errors
/// Any model, fit or I/O error.
pub fn run(out_dir: &str) -> Result<String, String> {
    let mut log = String::new();
    let t0 = std::time::Instant::now();
    let say = |log: &mut String, s: String| {
        println!("[{:7.1} s] {s}", t0.elapsed().as_secs_f64());
        let _ = writeln!(log, "{s}");
    };
    let rows = dataset::load_committed();
    let held: Vec<_> = rows
        .iter()
        .filter(|r| r.split == DpSplit::HeldOut)
        .collect();
    let hx: Vec<DoublePipeInput> = held.iter().map(|r| r.input).collect();
    let truth: Vec<f64> = held
        .iter()
        .map(|r| r.output.ua.get::<watt_per_kelvin>())
        .collect();
    let grid = artefacts::grid();

    // MFK, UA only.
    let mf = mfk_dataset()?;
    let mfk = Surrogate::fit_multi(
        &SurrogateSpec::CoKriging {
            config: MfkConfig::smt_defaults(),
        },
        &mf,
    )
    .map_err(|e| e.to_string())?;
    let mfk_at = |x: &DoublePipeInput| -> Result<(f64, f64), String> {
        let (m, s) = mfk.predict_si(&x.to_si()).map_err(|e| e.to_string())?;
        Ok((m[0], s.ok_or("MFK gave no sd")?[0]))
    };
    let mh: Vec<(f64, f64)> = hx.iter().map(&mfk_at).collect::<Result<_, _>>()?;
    let cov2 = mh
        .iter()
        .zip(&truth)
        .filter(|((m, s), t)| (*t - m).abs() <= 2.0 * s)
        .count();
    say(&mut log, format!("MFK (UA only): held-out UA relative RMSE {:.2} % (gh:#1190 record: 3.0 %), 2 sigma coverage {cov2}/8", 100.0 * rel_rmse(&mh.iter().map(|p| p.0).collect::<Vec<_>>(), &truth)));
    // The design sweep through the twin engine's API (gh:#1181).
    let mfk_grid: Vec<(f64, f64)> = cokriging::sweep(&mfk, &grid)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| {
            let sd = p.std_dev.ok_or("MFK gave no sd")?;
            Ok((p.mean.ua.get::<watt_per_kelvin>(), sd.ua.get::<watt_per_kelvin>()))
        })
        .collect::<Result<_, String>>()?;
    say(
        &mut log,
        format!("MFK on the {}-point grid done", grid.len()),
    );

    // KOH.
    let data = ua_dataset(&rows).map_err(|e| e.to_string())?;
    let design: Vec<DoublePipeInput> = rows.iter().map(|r| r.input).collect();
    let cal = KohCalibration::calibrate(
        TuasOneDUa,
        &data,
        &design,
        koh_spec(KohForm::WithDiscrepancy),
    )
    .map_err(|e| e.to_string())?;
    say(
        &mut log,
        format!(
            "KOH calibrated: m = {}, n = {}, {} TMCMC stages",
            cal.model().data.m(),
            cal.model().data.n(),
            cal.posterior().stages
        ),
    );
    let est = cal.theta_estimate().map_err(|e| e.to_string())?;
    let mean_theta = *est.point();
    let mean_si = Quantities::to_si(&mean_theta);
    let coords = cal.theta_coordinates();
    say(
        &mut log,
        format!(
            "posterior-mean theta (SI: theta_h, theta_c, r_f, f_h, f_c): {mean_si:?}; {} samples",
            coords.len()
        ),
    );

    let hp = cal
        .predict(&hx, STRIDE, true, None)
        .map_err(|e| e.to_string())?;
    let med: Vec<f64> = hp
        .iter()
        .map(|p| p.median.ua.get::<watt_per_kelvin>())
        .collect();
    let c95 = hp
        .iter()
        .zip(&truth)
        .filter(|(p, t)| {
            p.lower95.ua.get::<watt_per_kelvin>() <= **t
                && **t <= p.upper95.ua.get::<watt_per_kelvin>()
        })
        .count();
    let lf0: Vec<f64> = hx
        .iter()
        .map(|x| ua(x, &Theta::nominal()))
        .collect::<Result<_, _>>()?;
    let lf1: Vec<f64> = hx
        .iter()
        .map(|x| ua(x, &mean_theta))
        .collect::<Result<_, _>>()?;
    say(
        &mut log,
        format!(
            "KOH held-out UA relative RMSE {:.2} %, 95 % coverage {c95}/8 (gh:#1180 record: 1.28 %, 8/8); 1-D alone uncalibrated {:.2} % (record 8.95 %), at posterior-mean theta without delta {:.2} % (record 6.30 %)",
            100.0 * rel_rmse(&med, &truth),
            100.0 * rel_rmse(&lf0, &truth),
            100.0 * rel_rmse(&lf1, &truth)
        ),
    );
    for ((r, p), t) in held.iter().zip(&hp).zip(&truth) {
        say(
            &mut log,
            format!(
                "  {}: CFD {t:.5} W/K, KOH median {:.5} [{:.5}, {:.5}], delta {:+.4} ± {:.4}",
                r.id,
                p.median.ua.get::<watt_per_kelvin>(),
                p.lower95.ua.get::<watt_per_kelvin>(),
                p.upper95.ua.get::<watt_per_kelvin>(),
                p.delta_mean,
                p.delta_sd
            ),
        );
    }

    let mut out_rows = Vec::with_capacity(grid.len());
    for (c, chunk) in grid.chunks(CHUNK).enumerate() {
        let preds = cal
            .predict(chunk, STRIDE, true, None)
            .map_err(|e| e.to_string())?;
        for (k, (x, p)) in chunk.iter().zip(&preds).enumerate() {
            let (mm, ms) = mfk_grid[c * CHUNK + k];
            out_rows.push(GridRow {
                x: x.to_si(),
                ua_nominal: ua(x, &Theta::nominal())?,
                ua_post_mean: ua(x, &mean_theta)?,
                koh_median: p.median.ua.get::<watt_per_kelvin>(),
                koh_lower95: p.lower95.ua.get::<watt_per_kelvin>(),
                koh_upper95: p.upper95.ua.get::<watt_per_kelvin>(),
                koh_mean_ln: p.mean,
                koh_sd_ln: p.sd,
                delta_mean: p.delta_mean,
                delta_sd: p.delta_sd,
                mfk_mean: mm,
                mfk_sd: ms,
            });
        }
        if c % 5 == 0 {
            say(
                &mut log,
                format!("KOH grid chunk {}/{}", c + 1, grid.len().div_ceil(CHUNK)),
            );
        }
    }
    // The grid's own values at the held-out x (joint chunks vs the 8-point call).
    let mut worst = 0.0_f64;
    for (r, p) in held.iter().zip(&hp) {
        if let Some(g) = out_rows.iter().find(|g| g.x == r.input.to_si()) {
            worst = worst.max((g.koh_median / p.median.ua.get::<watt_per_kelvin>() - 1.0).abs());
        }
    }
    say(&mut log, format!("grid KOH median at the 8 held-out x vs the 8-point call: worst relative difference {worst:.2e}"));
    let ds: Vec<f64> = out_rows.iter().map(|r| r.delta_mean).collect();
    say(
        &mut log,
        format!(
            "delta(x) on ln UA over the grid: {:+.4} … {:+.4}",
            ds.iter().copied().fold(f64::INFINITY, f64::min),
            ds.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        ),
    );

    let head = header_lines();
    let mut g = head.clone();
    for l in &log.lines().collect::<Vec<_>>() {
        let _ = writeln!(g, "# {l}");
    }
    let _ = writeln!(g, "{GRID_HEADER}");
    for r in &out_rows {
        let _ = writeln!(g, "{}", r.to_csv());
    }
    let mut th = head;
    let _ = writeln!(th, "# Every {THIN}th of the {} TMCMC posterior samples of the gh:#1180 calibration (with delta).", coords.len());
    let _ = writeln!(
        th,
        "# {MEAN_TAG} {}",
        mean_si
            .iter()
            .map(|v| format!("{v}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    th.push_str(&theta_rows(&coords));
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    std::fs::write(format!("{out_dir}/koh_mfk_grid.csv"), g).map_err(|e| e.to_string())?;
    std::fs::write(format!("{out_dir}/koh_theta_samples.csv"), th).map_err(|e| e.to_string())?;
    say(
        &mut log,
        format!(
            "wrote {out_dir}/koh_mfk_grid.csv and koh_theta_samples.csv; total {:.0} s",
            t0.elapsed().as_secs_f64()
        ),
    );
    std::fs::write(format!("{out_dir}/generate_log.txt"), &log).map_err(|e| e.to_string())?;
    Ok(log)
}

/// Provenance lines common to both artefacts.
#[must_use]
pub fn header_lines() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# gh:#1182 (rung 5 of epic gh:#1177): precomputed artefacts of the double-pipe twin demo (outram-park-digital-twin-engine double_pipe_twin_web; dhoby-ghaut before 2026-11-05).");
    let _ = writeln!(s, "# Generated by: cargo run --release -p outram-park-digital-twin-engine --example double_pipe_twin_web -- --generate-artefacts");
    let _ = writeln!(s, "# KOH: raffles::fidelity::koh::KohCalibration with outram_park_digital_twin_engine::multifidelity::double_pipe::koh::koh_spec(WithDiscrepancy) on the 36 CFD training rows' ln UA (gh:#1180); predictive stride {STRIDE}, with observation error, chunks of {CHUNK}.");
    let _ = writeln!(s, "# MFK: raffles Surrogate::fit_multi CoKriging(MfkConfig::smt_defaults()), UA only, lf_grid.csv (375 TUAS rows, nominal theta) + 36 CFD training rows (gh:#1190).");
    let _ = writeln!(s, "# 1-D: one_d::ua (TUAS nodal conductances, march-free) at nominal theta and at the posterior-mean theta (no delta).");
    s
}

/// The thinned θ rows, header first: `[ln θ_h, ln θ_c, r_f]` per sample.
#[must_use]
pub fn theta_rows(coords: &[Vec<f64>]) -> String {
    let mut s = String::from("ln_theta_h,ln_theta_c,r_f_m2K_per_W\n");
    for c in coords.iter().step_by(THIN) {
        let _ = writeln!(s, "{},{},{}", c[0], c[1], c[2]);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cheap halves of the generator: the UA-only MFK dataset has
    /// gh:#1190's shape, the RMSE is the records' definition, and the θ
    /// writer's output reads back through the artefact parser.
    #[test]
    fn generator_pieces_are_consistent() {
        let mf = mfk_dataset().expect("dataset");
        assert_eq!(mf.levels()[0].counts(), (375, 0));
        assert_eq!(mf.highest().counts(), (36, 8));
        assert!((rel_rmse(&[1.1, 0.9], &[1.0, 1.0]) - 0.1).abs() < 1e-15);
        let coords: Vec<Vec<f64>> = (0..25).map(|i| vec![0.1 * i as f64, -0.2, 1e-4]).collect();
        let text = format!(
            "{}# {MEAN_TAG} 1.2,1.4,5e-4,1,1\n{}",
            header_lines(),
            theta_rows(&coords)
        );
        let p = artefacts::parse_theta(&text).expect("parses");
        assert_eq!(p.samples.len(), 3);
        assert_eq!(p.samples[1], [1.0, -0.2, 1e-4]);
        assert_eq!(p.mean_si, [1.2, 1.4, 5e-4, 1.0, 1.0]);
        assert!(ua(&artefacts::grid()[0], &Theta::nominal()).unwrap() > 0.0);
    }
}
