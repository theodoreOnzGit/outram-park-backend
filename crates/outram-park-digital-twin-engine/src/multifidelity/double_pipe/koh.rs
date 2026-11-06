//! Rung 3 of gh:#1177 (gh:#1180): **Kennedy–O'Hagan calibration of the
//! TUAS 1-D double pipe against the CFD**, with a GP discrepancy δ(x), via
//! `raffles::fidelity::koh` (SEPIA's formulation, translated and
//! differential-tested in raffles).
//!
//! - **What is calibrated:** θ_h and θ_c (the Nusselt multipliers, on ln)
//!   and r_f (the wall resistance), against the **36 CFD training rows'
//!   ln UA**. The friction multipliers do not enter UA and stay at 1.
//! - **Uncalibrated value:** θ = (1, 1, 0), [`Theta::nominal`].
//! - **The low fidelity seen by the emulator** is [`TuasOneDUa`]: TUAS's
//!   UA without the march ([`one_d::ua`], bit-identical to the marched UA).
//! - **Kept out of the calibration:** the 8 held-out CFD rows, the CFD's own
//!   Nusselt numbers, the literature annulus ratio, and the marched duty Q.
//!
//! The settings ([`koh_spec`]) and the predictions were fixed before the
//! run: `crates/raffles/verification_and_validation/koh_calibration_gh1180.md`
//! §4; results there and in
//! `verification_and_validation/double_pipe_koh_gh1180.md`.
use petir::mathf::RealMath;
use raffles::bayesian::koh::{KohForm, KohFree, KohSampleConfig};
use raffles::fidelity::koh::{CalibratedParameter, KohSpec, ObservationError, Scale, ThetaPrior};
use raffles::fidelity::{Dataset, Fidelity, ParameterSpace, ParametricModel, Split as RafflesSplit};
use raffles::{RafflesError, Result};
use uom::si::f64::{Ratio, ThermalConductance};
use uom::si::ratio::ratio;

use super::dataset::{self, DatasetRow};
use super::fidelity::{to_raffles_split, TuasOneD, CFD_RANK};
use super::{fouling_si, one_d, DoublePipeInput, Theta};

raffles::quantities! {
    /// The calibration output: the overall conductance alone.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct UaOutput {
        /// UA (CFD `UA_local`; 1-D from TUAS's nodal conductances).
        pub ua: ThermalConductance,
    }
}

/// TUAS's UA at (x, θ) without the march ([`one_d::ua`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TuasOneDUa;

impl ParametricModel for TuasOneDUa {
    type Input = DoublePipeInput;
    type Theta = Theta;
    type Output = UaOutput;

    /// [`TuasOneD`]'s space.
    fn parameter_space(&self) -> ParameterSpace<Theta> {
        TuasOneD.parameter_space()
    }

    fn evaluate(&self, x: &DoublePipeInput, theta: &Theta) -> Result<UaOutput> {
        one_d::ua(x, theta).map(|ua| UaOutput { ua }).map_err(|e| RafflesError::ModelEvaluation { model: "TUAS 1-D UA".to_string(), reason: e.to_string() })
    }
}

/// `rows`' UA as a CFD-fidelity dataset, each row keeping its split.
///
/// # Errors
/// [`Dataset::new`]'s.
pub fn ua_dataset(rows: &[DatasetRow]) -> Result<Dataset<DoublePipeInput, UaOutput>> {
    let x: Vec<DoublePipeInput> = rows.iter().map(|r| r.input).collect();
    let y: Vec<UaOutput> = rows.iter().map(|r| UaOutput { ua: r.output.ua }).collect();
    let split: Vec<RafflesSplit> = rows.iter().map(|r| to_raffles_split(r.split)).collect();
    Dataset::new(&x, &y, &split, Fidelity::new(CFD_RANK, "double-pipe CHT CFD (gh:#1156), UA only"), dataset::PROVENANCE)
}

/// θ at the design range's ends: ln θ_h ∈ [−0.6, 0.6], ln θ_c ∈ [−0.9,
/// 0.9] (±3σ of each prior), r_f ∈ [0, 1e-3] m² K/W; friction at 1.
#[must_use]
pub fn theta_range() -> (Theta, Theta) {
    let r = |v: f64| Ratio::new::<ratio>(v);
    let mut lo = Theta::nominal();
    lo.nu_mult_hot = r((-0.6_f64).r_exp());
    lo.nu_mult_cold = r((-0.9_f64).r_exp());
    lo.wall_resistance = fouling_si(0.0);
    let mut hi = Theta::nominal();
    hi.nu_mult_hot = r(0.6_f64.r_exp());
    hi.nu_mult_cold = r(0.9_f64.r_exp());
    hi.wall_resistance = fouling_si(1.0e-3);
    (lo, hi)
}

/// The calibration's settings, as fixed in the V&V record §4 before the
/// run: priors ln θ_h ~ N(0, 0.2²), ln θ_c ~ N(0, 0.3²), r_f flat; inputs
/// (ln Re_hot, ln Re_cold, ln k_wall, L); output ln UA; 6 low-fidelity runs
/// per design x (seed 1180); the CFD error ≈ 1.05 % in UA (gh:#1178's mesh
/// check) as λ_Os ~ Gamma(2, mean 1/σ²); TMCMC 600 × 10 moves for the
/// emulator and 2 000 × 10 for the calibration.
#[must_use]
pub fn koh_spec(form: KohForm) -> KohSpec<Theta> {
    let (theta_lower, theta_upper) = theta_range();
    let c = |name: &str, scale, prior| CalibratedParameter { name: name.to_string(), scale, prior };
    KohSpec {
        calibrated: vec![
            c("nu_mult_hot", Scale::Log, ThetaPrior::Normal { mean: 0.0, sd: 0.2 }),
            c("nu_mult_cold", Scale::Log, ThetaPrior::Normal { mean: 0.0, sd: 0.3 }),
            c("wall_resistance", Scale::Linear, ThetaPrior::Uniform),
        ],
        theta_lower,
        theta_upper,
        theta_fixed: Theta::nominal(),
        input_scales: vec![Scale::Log, Scale::Log, Scale::Log, Scale::Linear],
        output_scale: Scale::Log,
        runs_per_input: 6,
        design_seed: 1180,
        observation_error: Some(ObservationError { sd: 0.0105, shape: 2.0 }),
        form,
        emulator_sampler: KohSampleConfig { free: KohFree::All, population: 600, chain_length: 10, seed: 1180 },
        calibration_sampler: KohSampleConfig { free: KohFree::CalibrationOnly, population: 2000, chain_length: 10, seed: 1181 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multifidelity::double_pipe::design::Split;
    use crate::multifidelity::double_pipe::DoublePipeOutput;
    use raffles::fidelity::koh::KohCalibration;
    use raffles::fidelity::{CorrectedModel, Quantities};
    use uom::si::thermal_conductance::watt_per_kelvin;

    fn stats(v: &[f64]) -> (f64, f64, f64, f64, f64) {
        let n = v.len() as f64;
        let m = v.iter().sum::<f64>() / n;
        let sd = (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt();
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        let q = |p: f64| s[((p * (n - 1.0)).round()) as usize];
        (m, sd, q(0.025), q(0.5), q(0.975))
    }

    fn rel_rmse(pred: &[f64], truth: &[f64]) -> f64 {
        let n = truth.len() as f64;
        (pred.iter().zip(truth).map(|(a, b)| (a - b) * (a - b)).sum::<f64>() / n).sqrt() / (truth.iter().map(|t| t.abs()).sum::<f64>() / n)
    }

    /// **Methodology:** `crates/raffles/verification_and_validation/koh_calibration_gh1180.md`
    /// §4 (settings [`koh_spec`], predictions K1–K12, all fixed before the
    /// run). Calibrates on the 36 training rows' ln UA; reports θ's
    /// posterior, δ at the training x, the held-out predictive (median,
    /// 95 % interval, coverage, RMSE), the ablation, the no-δ contrast and
    /// the independent checks. The gates are only that every number exists
    /// and is finite: the numbers are reported, hits and misses alike.
    ///
    /// **Result (2026-11-04)**, 1 160 s (full table:
    /// `verification_and_validation/double_pipe_koh_gh1180.md`): posterior
    /// ln θ_h +0.164 ± 0.116, ln θ_c +0.336 ± 0.171, r_f 5.4e-4 ± 2.5e-4
    /// m² K/W, corr(h, c) −0.70. **Held-out UA RMSE 1.28 %, 95 % coverage
    /// 8/8** (MFK 3.0 %); the 1-D alone 8.95 % uncalibrated, 6.30 % at the
    /// posterior mean. Ablation, 1-D alone / with δ: θ_h −1.40 / −0.05
    /// points, θ_c +2.27 / +0.42, r_f +11.03 / −0.14. No δ: ln θ_c 0.777,
    /// r_f 8.5e-4, RMSE 1.80 %. θ_c's median 1.39 lies inside the CFD's own
    /// annulus Nu ratio range (1.26–1.58), θ_h's 1.18 outside the tube's
    /// (0.88–1.08). Marched Q at the posterior-mean θ without δ: +0.7 … +6.7 %
    /// (uncalibrated −2.4 … −5.3 %). Predictions met: K1, K4, K7, K9; missed:
    /// K2, K3, K5, K6 (better than predicted), K8, K10 (by 0.6 %), K11 (θ_h),
    /// K12. **θ is only partly identified: δ absorbs θ.**
    #[test]
    #[cfg_attr(not(feature = "long-tests"), ignore = "runtime > 5 min (long-tests)")]
    fn koh_calibration_of_the_double_pipe() {
        let rows = dataset::load_committed();
        let data = ua_dataset(&rows).unwrap();
        assert_eq!(data.counts(), (36, 8));
        let design: Vec<DoublePipeInput> = rows.iter().map(|r| r.input).collect();
        let t0 = std::time::Instant::now();
        let cal = KohCalibration::calibrate(TuasOneDUa, &data, &design, koh_spec(KohForm::WithDiscrepancy)).unwrap();
        println!("KOH: m = {}, n = {}, {} stages, {:.0} s", cal.model().data.m(), cal.model().data.n(), cal.posterior().stages, t0.elapsed().as_secs_f64());
        let e = cal.emulator();
        println!("emulator plug-in: beta_u {:?}, lam_uz {:.4}, lam_ws {:.1}, lam_wos {:.1}; y mean {:.4} sd {:.4}", e.beta_u, e.lam_uz, e.lam_ws, e.lam_wos, cal.model().data.y_mean, cal.model().data.y_sd);

        // θ posterior (K1-K4).
        let theta_report = |c: &KohCalibration<TuasOneDUa>, label: &str| {
            let t = c.theta_coordinates();
            let names = ["ln theta_h", "ln theta_c", "r_f [m2 K/W]"];
            let cols: Vec<Vec<f64>> = (0..3).map(|k| t.iter().map(|r| r[k]).collect()).collect();
            for k in 0..3 {
                let (m, sd, lo, med, hi) = stats(&cols[k]);
                println!("{label} {}: mean {m:.4e} sd {sd:.4e} median {med:.4e} 95% [{lo:.4e}, {hi:.4e}]", names[k]);
            }
            let corr = |a: &[f64], b: &[f64]| {
                let (ma, sa, ..) = stats(a);
                let (mb, sb, ..) = stats(b);
                a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum::<f64>() / ((a.len() as f64 - 1.0) * sa * sb)
            };
            println!("{label} corr(h,c) {:.3} corr(h,r_f) {:.3} corr(c,r_f) {:.3}", corr(&cols[0], &cols[1]), corr(&cols[0], &cols[2]), corr(&cols[1], &cols[2]));
            let lam: Vec<f64> = c.posterior().samples.iter().map(|p| p.lam_os).collect();
            let sig: Vec<f64> = lam.iter().map(|l| c.model().data.y_sd / l.sqrt()).collect();
            let (m, _, lo, _, hi) = stats(&sig);
            println!("{label} eps sd on ln UA: mean {m:.4} 95% [{lo:.4}, {hi:.4}]");
            cols
        };
        let cols = theta_report(&cal, "KOH");
        for v in cols.iter().flatten() {
            assert!(v.is_finite());
        }

        // δ over the design space (K5), at the 36 training x.
        let train_x: Vec<DoublePipeInput> = rows.iter().filter(|r| r.split == Split::Training).map(|r| r.input).collect();
        let dtrain = cal.predict(&train_x, 10, false, None).unwrap();
        let mut small = 0;
        for (r, p) in rows.iter().filter(|r| r.split == Split::Training).zip(&dtrain) {
            println!("delta {}: x {:?} delta {:+.4} ± {:.4}", r.id, r.input.to_si(), p.delta_mean, p.delta_sd);
            if p.delta_mean.abs() < 0.05 {
                small += 1;
            }
        }
        println!("|delta| < 0.05 at {small}/36");

        // Held-out predictive (K6, K7).
        let held: Vec<&DatasetRow> = rows.iter().filter(|r| r.split == Split::HeldOut).collect();
        let hx: Vec<DoublePipeInput> = held.iter().map(|r| r.input).collect();
        let truth: Vec<f64> = held.iter().map(|r| r.output.ua.get::<watt_per_kelvin>()).collect();
        let report = |label: &str, preds: &[raffles::fidelity::koh::KohPrediction<UaOutput>]| -> f64 {
            let med: Vec<f64> = preds.iter().map(|p| p.median.ua.get::<watt_per_kelvin>()).collect();
            let (mut c95, mut c2s) = (0, 0);
            for ((p, t), r) in preds.iter().zip(&truth).zip(&held) {
                let (lo, hi) = (p.lower95.ua.get::<watt_per_kelvin>(), p.upper95.ua.get::<watt_per_kelvin>());
                let inside = lo <= *t && *t <= hi;
                let z = (t.ln() - p.mean) / p.sd;
                c95 += usize::from(inside);
                c2s += usize::from(z.abs() <= 2.0);
                println!("{label} {}: CFD {t:.5} median {:.5} ({:+.2} %) 95% [{lo:.5}, {hi:.5}] {} z {z:+.2} delta {:+.4}", r.id, p.median.ua.get::<watt_per_kelvin>(), 100.0 * (p.median.ua.get::<watt_per_kelvin>() / t - 1.0), if inside { "in" } else { "OUT" }, p.delta_mean);
            }
            let e = rel_rmse(&med, &truth);
            println!("{label}: held-out UA relative RMSE {:.2} %, 95 % coverage {c95}/8, 2 sigma coverage {c2s}/8", 100.0 * e);
            assert!(e.is_finite());
            e
        };
        let base = report("KOH", &cal.predict(&hx, 5, true, None).unwrap());

        // The 1-D model alone (no δ), uncalibrated and at the posterior mean.
        let est = cal.theta_estimate().unwrap();
        let lf_rmse = |th: &Theta| -> f64 {
            let p: Vec<f64> = hx.iter().map(|x| one_d::ua(x, th).unwrap().get::<watt_per_kelvin>()).collect();
            rel_rmse(&p, &truth)
        };
        let mean_theta = *est.point();
        let (lf0, lf1) = (lf_rmse(&Theta::nominal()), lf_rmse(&mean_theta));
        println!("1-D alone: uncalibrated {:.2} %, at posterior-mean theta {:.2} % (theta {:?})", 100.0 * lf0, 100.0 * lf1, Quantities::to_si(&mean_theta));

        // Ablation (K8, K9).
        for name in ["nu_mult_hot", "nu_mult_cold", "wall_resistance"] {
            let mut th = mean_theta;
            match name {
                "nu_mult_hot" => th.nu_mult_hot = Ratio::new::<ratio>(1.0),
                "nu_mult_cold" => th.nu_mult_cold = Ratio::new::<ratio>(1.0),
                _ => th.wall_resistance = fouling_si(0.0),
            }
            let lf = lf_rmse(&th);
            let k = report(&format!("reset {name}"), &cal.predict(&hx, 5, true, Some(name)).unwrap());
            println!("ablation {name}: 1-D alone {:.2} % ({:+.2} points); with delta {:.2} % ({:+.2} points)", 100.0 * lf, 100.0 * (lf - lf1), 100.0 * k, 100.0 * (k - base));
        }

        // No-δ contrast (K10).
        let nod = cal.recalibrate(KohForm::NoDiscrepancy).unwrap();
        theta_report(&nod, "no-delta");
        report("no-delta", &nod.predict(&hx, 5, true, None).unwrap());

        // Independent checks (K11, K12): the CFD's own Nusselt ratios, and
        // the marched duty at the posterior-mean θ.
        let (mut rh, mut rc) = (Vec::new(), Vec::new());
        for r in &rows {
            let (nt, ns) = one_d::textbook_nusselt(&r.input).unwrap();
            rh.push(r.output.nu_hot.get::<ratio>() / nt.get::<ratio>());
            rc.push(r.output.nu_cold.get::<ratio>() / ns.get::<ratio>());
        }
        let mm = |v: &[f64]| (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        let (h_med, c_med) = (stats(&cols[0]).3.exp(), stats(&cols[1]).3.exp());
        println!("CFD Nu ratio tube {:?} vs theta_h median {h_med:.4}; annulus {:?} vs theta_c median {c_med:.4}", mm(&rh), mm(&rc));
        let corrected = CorrectedModel::new(TuasOneD, est, None);
        let q: Vec<(f64, f64)> = std::thread::scope(|s| {
            let hs: Vec<_> = hx.chunks(2).map(|ch| {
                let corrected = &corrected;
                s.spawn(move || ch.iter().map(|x| (corrected.predict(x).unwrap().mean.q.value, one_d::evaluate(x, &Theta::nominal()).q.value)).collect::<Vec<_>>())
            }).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        for ((r, (qc, q0)), _) in held.iter().zip(&q).zip(0..) {
            let t: &DoublePipeOutput = &r.output;
            println!("Q {}: CFD {:.4} W, calibrated 1-D {qc:.4} W ({:+.2} %), uncalibrated {q0:.4} W ({:+.2} %)", r.id, t.q.value, 100.0 * (qc / t.q.value - 1.0), 100.0 * (q0 / t.q.value - 1.0));
            assert!(qc.is_finite());
        }
        println!("total {:.0} s", t0.elapsed().as_secs_f64());
    }

    /// The UA dataset keeps the committed split; the UA model is TUAS's
    /// march-free UA; the spec's θ range holds nominal θ.
    #[test]
    fn the_ua_pair_and_spec_are_consistent() {
        let rows = dataset::load_committed();
        let d = ua_dataset(&rows).unwrap();
        assert_eq!(d.counts(), (36, 8));
        let x = rows[0].input;
        assert_eq!(TuasOneDUa.evaluate(&x, &Theta::nominal()).unwrap().ua, one_d::ua(&x, &Theta::nominal()).unwrap());
        assert!(TuasOneDUa.evaluate(&DoublePipeInput::si(30.0, 15.0, 3.0, 0.1), &Theta::nominal()).is_err());
        assert!(TuasOneDUa.parameter_space().contains(&Theta::nominal()));
        let (lo, hi) = theta_range();
        let s = koh_spec(KohForm::WithDiscrepancy);
        assert_eq!(s.calibrated.len(), 3);
        assert!(lo.nu_mult_cold < Theta::nominal().nu_mult_cold && hi.nu_mult_cold > Theta::nominal().nu_mult_cold);
        assert_eq!(s.theta_fixed, Theta::nominal());
    }
}
