//! The double-pipe pair as a two-level co-kriging problem (gh:#1190; the
//! rung-4 application of epic gh:#1177, gh:#1181): the TUAS 1-D model at
//! nominal θ on the [`lf_grid`](super::lf_grid) as the low fidelity, the
//! committed CFD's training rows as the high fidelity, through
//! `raffles::fidelity::MultiFidelityDataset` and `Surrogate::fit_multi`
//! (SMT's MFK, `SurrogateSpec::CoKriging`).
//!
//! No physics here: the two levels are other modules' outputs, tabulated.
//! V&V record, with the settings and predictions fixed before the run:
//! `crates/raffles/verification_and_validation/smt_kriging_gh1190.md`,
//! "The double pipe".
use raffles::fidelity::{Dataset, Fidelity, MultiFidelityDataset, Split as RafflesSplit};
use raffles::Result;

use super::fidelity::{committed_dataset, ONE_D_RANK};
use super::lf_grid;
use super::{DoublePipeInput, DoublePipeOutput};

/// Provenance of the low-fidelity level.
pub const LF_PROVENANCE: &str = "TUAS 1-D double pipe (one_d::try_evaluate) at nominal theta on the 375-point lf_grid; committed as verification_and_validation/double_pipe_mfk_gh1190/lf_grid.csv (gh:#1190)";

/// The committed low-fidelity grid as a [`Dataset`]: every row training
/// (the low fidelity is not the check).
///
/// # Errors
/// A malformed committed CSV (pinned by `lf_grid`'s test), or [`Dataset::new`]'s.
pub fn low_fidelity_dataset() -> Result<Dataset<DoublePipeInput, DoublePipeOutput>> {
    let rows = lf_grid::load_committed().map_err(|e| raffles::RafflesError::ModelEvaluation { model: "lf_grid.csv".to_string(), reason: e })?;
    let x: Vec<DoublePipeInput> = rows.iter().map(|r| r.0).collect();
    let y: Vec<DoublePipeOutput> = rows.iter().map(|r| r.1).collect();
    Dataset::new(&x, &y, &vec![RafflesSplit::Training; rows.len()], Fidelity::new(ONE_D_RANK, "TUAS 1-D, nominal theta"), LF_PROVENANCE)
}

/// `[low fidelity, CFD]`: the CFD keeps its stored split (36 training, 8
/// held out), which `fit_multi` respects.
///
/// # Errors
/// The two datasets' constructors.
pub fn multi_fidelity_dataset() -> Result<MultiFidelityDataset<DoublePipeInput, DoublePipeOutput>> {
    MultiFidelityDataset::new(vec![low_fidelity_dataset()?, committed_dataset()?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use raffles::fidelity::{KrigingRoute, Quantities, Surrogate, SurrogateSpec, ValidationReport};
    use raffles::surrogate::{MfkConfig, SmtKrgConfig};

    /// **Methodology** (V&V record `smt_kriging_gh1190.md`, "The double
    /// pipe", settings fixed before the run): SMT's MFK with its defaults
    /// (squar_exp, constant trend and ρ, nugget, θ bounds, the port's
    /// default 10-point multistart), one per output, raw SI inputs, fitted by
    /// `Surrogate::fit_multi` to the 375 TUAS rows (nominal θ) and the 36 CFD
    /// training rows; scored on the 8 held-out CFD rows by
    /// `ValidationReport::of_multi` (RMSE, max error, 2σ coverage). The
    /// comparator SMT KRG (defaults) is fitted to the 36 CFD training rows
    /// alone. Gates: the fits exist and every error is finite; the numbers are
    /// reported, nothing is tuned.
    ///
    /// **Predictions** D1–D7 (record). **Result (2026-11-04)**, held-out
    /// RMSE relative to the mean |y|: MFK UA 3.0 %, Q 0.56 %, ε 0.0086,
    /// outlet T 0.38 / 0.20 K, Δp 0.56 / 0.47 %, Nu 2.8 / 1.7 %; KRG on the
    /// CFD alone UA 13.9 %, Q 13.0 %, ε 0.170, Δp 10.0 / 53.6 %. MFK beats
    /// KRG and the degree-1 polynomial (gh:#1194) on all nine outputs; its 2σ
    /// band holds 7–8 of 8 rows except Δp_hot (3/8, too narrow).
    ///
    /// **Runtime:** 1146 s on 2026-11-04 (MFK 1111 s: nine fits with 375
    /// low-fidelity rows), so it is behind `long-tests` (on by default) by
    /// the runtime criterion.
    #[test]
    #[cfg_attr(not(feature = "long-tests"), ignore = "runtime ~19 min (long-tests, runtime criterion)")]
    fn mfk_on_the_double_pipe_held_out_report() {
        let mf = multi_fidelity_dataset().unwrap();
        assert_eq!(mf.levels()[0].counts(), (375, 0));
        assert_eq!(mf.highest().counts(), (36, 8));
        let (_, hy) = mf.highest().held_out_si();
        let names: Vec<String> = DoublePipeOutput::channels().into_iter().map(|c| format!("{} [{}]", c.name, c.unit)).collect();
        let specs = [
            ("MFK (TUAS LF + CFD HF)", SurrogateSpec::CoKriging { config: MfkConfig::smt_defaults() }),
            ("KRG (CFD HF only)", SurrogateSpec::Kriging { route: KrigingRoute::Smt(SmtKrgConfig::smt_defaults()) }),
        ];
        for (label, spec) in &specs {
            let t0 = std::time::Instant::now();
            let s = Surrogate::fit_multi(spec, &mf).unwrap();
            let r = ValidationReport::of_multi(&s, spec, &mf, false).unwrap();
            let rmse = Quantities::to_si(&r.held_out_rmse.unwrap());
            let max = Quantities::to_si(&r.held_out_max_error.unwrap());
            let cov = r.coverage_2sigma.clone().unwrap();
            println!("{label} ({:.0} s): channel | held-out RMSE | relative | max |error| | 2σ coverage", t0.elapsed().as_secs_f64());
            for k in 0..names.len() {
                let mean = hy.iter().map(|y| y[k].abs()).sum::<f64>() / hy.len() as f64;
                println!("  {} | {:.4e} | {:.2} % | {:.4e} | {}/8", names[k], rmse[k], 100.0 * rmse[k] / mean, max[k], (cov[k] * 8.0).round());
                assert!(rmse[k].is_finite() && max[k].is_finite(), "{label} {}", names[k]);
            }
        }
    }
}
