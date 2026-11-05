//! gh:#1192's pilot: OpenDA's algorithms (ported into `raffles::openda`) on
//! the double pipe. **Synthetic truth only** (`RESPONSIBLE_USE.md`).
//!
//! - **(a)** OpenDA's EnKF on gh:#1191's twin experiment, beside FilterPy's
//!   EnKF: [`super::filtering::FilterKind::OpenDaEnsemble`] through
//!   [`super::filtering::run_experiment`] (same model, measurements, priors, N 50).
//! - **(b)** OpenDA's DUD calibrating θ = (ln θ_h, ln θ_c, r_f) on the 36 CFD
//!   training rows' ln UA with OpenDA's weighted cost, for comparison with
//!   gh:#1180's Kennedy–O'Hagan posterior: [`dud_calibration`].
//!
//! Method and predictions (written before this code):
//! `verification_and_validation/double_pipe_openda_gh1192.md`.
//!
//! **No new physics:** the 1-D model is TUAS's march-free UA
//! ([`one_d::ua`], gh:#1180's `TuasOneDUa`); this module composes it with
//! the ported optimiser.
use petir::mathf::RealMath;
use raffles::openda::{CostFunction, Dud, Predictor, StochVector, WeightedLeastSquares};
use raffles::{RafflesError, Result};
use uom::si::ratio::ratio;
use uom::si::thermal_conductance::watt_per_kelvin;

use super::dataset::{load_committed, DatasetRow};
use super::design::Split;
use super::{fouling_si, one_d, DoublePipeInput, Theta};

/// The CFD error on ln UA used as σ (gh:#1180's ≈ 1.05 % mesh estimate).
pub const SIGMA_LN_UA: f64 = 0.0105;

/// θ for a parameter vector `(ln θ_h, ln θ_c, r_f)`; friction at 1.
#[must_use]
pub fn theta_of(p: &[f64]) -> Theta {
    let mut t = Theta::nominal();
    t.nu_mult_hot = uom::si::f64::Ratio::new::<ratio>(p[0].r_exp());
    t.nu_mult_cold = uom::si::f64::Ratio::new::<ratio>(p[1].r_exp());
    t.wall_resistance = fouling_si(p[2]);
    t
}

/// ln UA of TUAS's 1-D model at each input, for parameters p.
#[derive(Debug, Clone, PartialEq)]
pub struct LnUaPredictor {
    inputs: Vec<DoublePipeInput>,
}

impl LnUaPredictor {
    /// Over `inputs`.
    #[must_use]
    pub fn new(inputs: Vec<DoublePipeInput>) -> Self {
        Self { inputs }
    }
}

impl Predictor for LnUaPredictor {
    fn predict(&self, p: &[f64]) -> Result<Vec<f64>> {
        let th = theta_of(p);
        self.inputs
            .iter()
            .map(|x| one_d::ua(x, &th).map(|u| u.get::<watt_per_kelvin>().r_ln()).map_err(|e| RafflesError::ModelEvaluation { model: "TUAS 1-D UA".into(), reason: e.to_string() }))
            .collect()
    }
}

/// The prior as OpenDA's parameter uncertainty: ln θ_h N(0, 0.2²), ln θ_c
/// N(0, 0.3²), r_f N(5e-4, (1e-3/√12)²) (gh:#1180's U[0, 1e-3] by its mean
/// and sd).
///
/// # Errors
/// Never (fixed shapes).
pub fn prior() -> Result<StochVector> {
    StochVector::independent(vec![0.0, 0.0, 5.0e-4], vec![0.2, 0.3, 1.0e-3 / 12.0_f64.sqrt()])
}

/// One DUD calibration's outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct DudCalibration {
    /// Best θ, `(ln θ_h, ln θ_c, r_f)`.
    pub theta: Vec<f64>,
    /// Its weighted cost.
    pub cost: f64,
    /// DUD's linearised standard deviations.
    pub std: Vec<f64>,
    /// Cost evaluations (each 36 TUAS UA evaluations).
    pub evaluations: usize,
    /// Outer iterations.
    pub iterations: usize,
    /// RMSE of ln UA over the training rows at θ.
    pub training_rmse_ln_ua: f64,
    /// Held-out UA relative RMSE of the 1-D model at θ (gh:#1180's metric).
    pub held_out_rel_rmse: f64,
}

fn split(rows: &[DatasetRow], s: Split) -> Vec<&DatasetRow> {
    rows.iter().filter(|r| r.split == s).collect()
}

/// gh:#1180's metric: √(mean (pred − truth)²) / mean |truth|.
#[must_use]
pub fn rel_rmse(pred: &[f64], truth: &[f64]) -> f64 {
    let n = truth.len() as f64;
    (pred.iter().zip(truth).map(|(a, b)| (a - b) * (a - b)).sum::<f64>() / n).sqrt() / (truth.iter().map(|t| t.abs()).sum::<f64>() / n)
}

/// DUD from the uncalibrated θ = (0, 0, 0) with search steps the prior sds,
/// `maxit` 10, OpenDA's other defaults (record §1(b)).
///
/// # Errors
/// TUAS's, or the optimiser's.
pub fn dud_calibration(background: bool) -> Result<DudCalibration> {
    let rows = load_committed();
    let train = split(&rows, Split::Training);
    let held = split(&rows, Split::HeldOut);
    let y: Vec<f64> = train.iter().map(|r| r.output.ua.get::<watt_per_kelvin>().r_ln()).collect();
    let pred = LnUaPredictor::new(train.iter().map(|r| r.input).collect());
    let mut f = WeightedLeastSquares::new(pred.clone(), StochVector::independent(y.clone(), vec![SIGMA_LN_UA; y.len()])?, prior()?, 0.5, background);
    let mut d = Dud::default();
    d.initialize(&mut f, &[0.0, 0.0, 0.0])?;
    d.max_iterations = 10;
    d.optimize(&mut f)?;
    let (cost, theta) = f.history().optimum().map(|(c, p)| (c, p.to_vec())).ok_or_else(|| RafflesError::InvalidParameter { parameter: "dud".into(), value: 0.0, reason: "no evaluation".into() })?;
    let fit = pred.predict(&theta)?;
    let training_rmse_ln_ua = (fit.iter().zip(&y).map(|(a, b)| (a - b) * (a - b)).sum::<f64>() / y.len() as f64).sqrt();
    let th = theta_of(&theta);
    let hp = held.iter().map(|r| one_d::ua(&r.input, &th).map(|u| u.get::<watt_per_kelvin>())).collect::<std::result::Result<Vec<f64>, _>>().map_err(|e| RafflesError::ModelEvaluation { model: "TUAS 1-D UA".into(), reason: e.to_string() })?;
    let ht: Vec<f64> = held.iter().map(|r| r.output.ua.get::<watt_per_kelvin>()).collect();
    Ok(DudCalibration {
        theta,
        cost,
        std: d.std_values().map(<[f64]>::to_vec).unwrap_or_default(),
        evaluations: f.history().costs.len(),
        iterations: d.iterations(),
        training_rmse_ln_ua,
        held_out_rel_rmse: rel_rmse(&hp, &ht),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multifidelity::double_pipe::filtering::{run_experiment, Experiment, FilterKind, Run, BURN_IN};

    fn line(e: Experiment, f: FilterKind, r: &Run) -> String {
        let last = r.trace.last().unwrap();
        format!(
            "{} {:?}: RMSE(31-200) {:.3e} bias {:+.3e} NEES {:.3} NIS {:.3} cov95 {:.3} final sigma {:.3e} final {:?}",
            e.label(),
            f,
            r.rmse_after_burn_in,
            r.bias_after_burn_in,
            r.nees_mean,
            r.nis_mean,
            r.coverage_95,
            last[2],
            r.final_mean
        )
    }

    /// **Pilot (a)** (record `double_pipe_openda_gh1192.md`, predictions
    /// OA1–OA6 written first): OpenDA's EnKF and FilterPy's EnKF (N 50
    /// each) on the four experiments of gh:#1191. Asserts only finite
    /// numbers and a deterministic rerun; the predictions are judged in the
    /// record, misses included.
    ///
    /// **Results (2026-11-04)**, r_f RMSE (steps 31–200) / NEES / NIS,
    /// OpenDA vs FilterPy EnKF: P-A 8.49e-6 / 1.25 / 1.83 vs 8.73e-6 / 1.33 /
    /// 1.85; P-B bias −4.374e-4 vs −4.373e-4; P-D 4.89e-4 / 23.5 / 1.74 vs
    /// 3.73e-4 / 10.4 / 1.78; P-C 2.31e-5 / 1.83 / 1.87 vs 3.17e-5 / 3.83 /
    /// 1.89. All 8 runs 2.4 s. **OA1–OA6 all met.**
    #[test]
    fn pilot_a_openda_enkf_beside_filterpy_enkf() {
        let t0 = std::time::Instant::now();
        let mut out = Vec::new();
        for e in Experiment::ALL {
            for f in [FilterKind::Ensemble, FilterKind::OpenDaEnsemble] {
                let r = run_experiment(e, f).unwrap();
                println!("{}", line(e, f, &r));
                assert!(r.rmse_all.is_finite() && r.nees_mean.is_finite() && r.nis_mean.is_finite());
                assert_eq!(r.trace.len(), crate::multifidelity::double_pipe::filtering::STEPS);
                out.push((e, f, r));
            }
        }
        let again = run_experiment(Experiment::PA, FilterKind::OpenDaEnsemble).unwrap();
        assert_eq!(again.trace, out[1].2.trace, "deterministic");
        assert!(BURN_IN < out[1].2.trace.len());
        println!("pilot (a) total {:.1} s", t0.elapsed().as_secs_f64());
    }

    /// **Pilot (b)** (record `double_pipe_openda_gh1192.md`, predictions
    /// OB1–OB6 written first): DUD on the 36 training rows' ln UA, without
    /// and with the background term, against gh:#1180's KOH posterior
    /// (mean and median; the mode was not recorded). Asserts only that DUD
    /// ran and its numbers are finite.
    ///
    /// **Results (2026-11-04):** no background: (ln θ_h −0.061, ln θ_c
    /// +2.161, r_f 1.67e-3), sd (0.0095, 1.08, 1.1e-4), stopped by the
    /// 10-iteration cap (24 evaluations), training ln UA RMSE 0.0279, held-out
    /// UA 1.49 %. With background: (−0.028, +1.128, 1.25e-3), held-out 1.78 %.
    /// KOH's no-δ mean gives training RMSE 0.0329. **Met OB3, OB4, OB5;
    /// missed OB1 (cap, not convergence), OB2, OB6**: θ_c and r_f run along
    /// an unidentified ridge without δ.
    #[test]
    fn pilot_b_dud_against_the_koh_posterior() {
        let t0 = std::time::Instant::now();
        let p = prior().unwrap();
        for bg in [false, true] {
            let c = dud_calibration(bg).unwrap();
            println!(
                "DUD background {bg}: theta (ln th_h {:+.4}, ln th_c {:+.4}, r_f {:.4e}) sd ({:.4}, {:.4}, {:.3e}) cost {:.4} after {} iterations / {} evaluations; training ln UA RMSE {:.4}; held-out UA rel RMSE {:.2} %",
                c.theta[0],
                c.theta[1],
                c.theta[2],
                c.std[0],
                c.std[1],
                c.std[2],
                c.cost,
                c.iterations,
                c.evaluations,
                c.training_rmse_ln_ua,
                100.0 * c.held_out_rel_rmse
            );
            assert!(c.theta.iter().chain(&c.std).all(|v| v.is_finite()) && c.cost.is_finite());
        }
        // gh:#1180's posterior mean (with delta), for the same metrics.
        let rows = load_committed();
        let train = split(&rows, Split::Training);
        let y: Vec<f64> = train.iter().map(|r| r.output.ua.get::<watt_per_kelvin>().r_ln()).collect();
        let pred = LnUaPredictor::new(train.iter().map(|r| r.input).collect());
        for (label, th) in [("uncalibrated", [0.0, 0.0, 0.0]), ("KOH with-delta mean", [0.164, 0.336, 5.43e-4]), ("KOH no-delta mean", [-0.025, 0.777, 8.51e-4])] {
            let fit = pred.predict(&th).unwrap();
            let rmse = (fit.iter().zip(&y).map(|(a, b)| (a - b) * (a - b)).sum::<f64>() / y.len() as f64).sqrt();
            let mut f = WeightedLeastSquares::new(pred.clone(), StochVector::independent(y.clone(), vec![SIGMA_LN_UA; y.len()]).unwrap(), p.clone(), 0.5, false);
            println!("{label} {th:?}: training ln UA RMSE {rmse:.4}, cost {:.4}", f.evaluate(&th).unwrap());
        }
        println!("pilot (b) total {:.1} s", t0.elapsed().as_secs_f64());
        assert!(rel_rmse(&[1.0], &[1.0]) == 0.0);
    }
}
