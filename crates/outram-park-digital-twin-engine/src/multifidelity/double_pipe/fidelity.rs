//! The double-pipe pair through `raffles::fidelity` (gh:#1194): the first
//! real implementer of its traits.
//!
//! - [`TuasOneD`] is the TUAS 1-D model as a
//!   [`ParametricModel`]: `x` = [`DoublePipeInput`], `θ` = [`Theta`],
//!   `y` = [`DoublePipeOutput`], evaluated by [`one_d::try_evaluate`]. No
//!   physics is added here; the adapter only maps the error type.
//! - [`committed_dataset`] is gh:#1178's committed CFD design grid as a
//!   [`Dataset`], **keeping the split stored in every row** (36 training, 8
//!   held-out, fixed before any CFD run) and [`dataset::PROVENANCE`].
//!
//! The three records derive `raffles::fidelity::Quantities` through
//! `raffles::quantities!` where they are defined (`super`). Their inherent
//! `DoublePipeInput::to_si` (an array) is the same numbers as the trait's
//! `Quantities::to_si` (a `Vec`): every field is a ratio, W/(m K) or m,
//! already SI.
//!
//! V&V: the tests below, recorded in
//! `crates/raffles/verification_and_validation/fidelity_interface_gh1194.md`.
use raffles::fidelity::{Dataset, Fidelity, ParameterSpace, ParametricModel, Split as RafflesSplit};
use raffles::{RafflesError, Result};
use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

use super::dataset::{self, DatasetRow};
use super::design::Split;
use super::{fouling_si, one_d, DoublePipeInput, DoublePipeOutput, Theta};

/// Fidelity rank of the TUAS 1-D model.
pub const ONE_D_RANK: u32 = 1;
/// Fidelity rank of the double-pipe CFD.
pub const CFD_RANK: u32 = 2;

/// The TUAS 1-D double pipe, `y = f_LF(x, θ)` ([`one_d::try_evaluate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TuasOneD;

impl TuasOneD {
    /// The bounds on θ. ~~**Provisional, for gh:#1180 to replace with its
    /// prior; no result in this crate depends on them.**~~ **CORRECTED
    /// 2026-11-04:** gh:#1180 did not replace them; its calibration states
    /// its own range and priors ([`super::koh::theta_range`],
    /// [`super::koh::koh_spec`]). These stay provisional, and still no
    /// result depends on them.
    ///
    /// - Nusselt and friction multipliers: [0.25, 4], a factor of 4 either
    ///   way around TUAS's textbook correlation (θ nominal = 1). Not from
    ///   literature: a deliberately wide placeholder.
    /// - Wall (fouling) resistance on the tube's outer area: [0, 1e-3]
    ///   m² K/W, the upper end of the water-service fouling factors tabulated
    ///   by Incropera & DeWitt, *Fundamentals of Heat and Mass Transfer*,
    ///   Table 11.1 (river water, 0.0002–0.001 m² K/W).
    #[must_use]
    pub fn theta_bounds() -> (Theta, Theta) {
        let r = |v: f64| Ratio::new::<ratio>(v);
        let lower = Theta {
            nu_mult_hot: r(0.25),
            nu_mult_cold: r(0.25),
            wall_resistance: fouling_si(0.0),
            friction_mult_hot: r(0.25),
            friction_mult_cold: r(0.25),
        };
        let upper = Theta {
            nu_mult_hot: r(4.0),
            nu_mult_cold: r(4.0),
            wall_resistance: fouling_si(1.0e-3),
            friction_mult_hot: r(4.0),
            friction_mult_cold: r(4.0),
        };
        (lower, upper)
    }
}

impl ParametricModel for TuasOneD {
    type Input = DoublePipeInput;
    type Theta = Theta;
    type Output = DoublePipeOutput;

    /// Uniform on [`TuasOneD::theta_bounds`].
    ///
    /// # Panics
    /// Never: the bounds are constants with lower < upper, pinned by
    /// `the_parameter_space_holds_nominal_theta`. Justified because the trait
    /// returns the space, not a `Result`.
    fn parameter_space(&self) -> ParameterSpace<Theta> {
        let (lo, hi) = Self::theta_bounds();
        match ParameterSpace::bounded(&lo, &hi) {
            Ok(s) => s,
            Err(e) => panic!("TuasOneD θ bounds: {e}"),
        }
    }

    fn evaluate(&self, x: &DoublePipeInput, theta: &Theta) -> Result<DoublePipeOutput> {
        one_d::try_evaluate(x, theta).map_err(|e| RafflesError::ModelEvaluation {
            model: "TUAS 1-D double pipe".to_string(),
            reason: e.to_string(),
        })
    }
}

/// gh:#1178's split as `raffles`' split.
#[must_use]
pub fn to_raffles_split(s: Split) -> RafflesSplit {
    match s {
        Split::Training => RafflesSplit::Training,
        Split::HeldOut => RafflesSplit::HeldOut,
    }
}

/// `rows` as a CFD-fidelity [`Dataset`], each row keeping its split.
///
/// # Errors
/// [`Dataset::new`]'s (no training row).
pub fn to_dataset(rows: &[DatasetRow]) -> Result<Dataset<DoublePipeInput, DoublePipeOutput>> {
    let x: Vec<DoublePipeInput> = rows.iter().map(|r| r.input).collect();
    let y: Vec<DoublePipeOutput> = rows.iter().map(|r| r.output).collect();
    let split: Vec<RafflesSplit> = rows.iter().map(|r| to_raffles_split(r.split)).collect();
    Dataset::new(
        &x,
        &y,
        &split,
        Fidelity::new(CFD_RANK, "double-pipe CHT CFD (gh:#1156)"),
        dataset::PROVENANCE,
    )
}

/// The committed CFD dataset ([`dataset::load_committed`], 44 rows) as a
/// [`Dataset`].
///
/// # Errors
/// [`to_dataset`]'s; never for the committed file (pinned by a test).
pub fn committed_dataset() -> Result<Dataset<DoublePipeInput, DoublePipeOutput>> {
    to_dataset(&dataset::load_committed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use raffles::fidelity::{
        CorrectedModel, Quantities, Surrogate, SurrogateSpec, ThetaEstimate, ValidationReport,
    };

    /// The committed dataset through the typed interface: 44 rows, the stored
    /// split (36 / 8), the provenance, and every row reading back equal to
    /// the parsed CSV row (the uom round trip is exact).
    #[test]
    fn the_committed_dataset_keeps_its_split_and_provenance() {
        let rows = dataset::load_committed();
        let d = committed_dataset().unwrap();
        assert_eq!(d.len(), 44);
        assert_eq!(d.counts(), (36, 8));
        assert_eq!(d.provenance(), dataset::PROVENANCE);
        assert_eq!(d.fidelity().rank, CFD_RANK);
        for (i, r) in rows.iter().enumerate() {
            let (x, y, s) = d.row(i).unwrap();
            assert_eq!(
                (x, y, s),
                (r.input, r.output, to_raffles_split(r.split)),
                "{}",
                r.id
            );
            assert_eq!(Quantities::to_si(&x), r.input.to_si().to_vec());
        }
        assert_eq!(to_raffles_split(Split::HeldOut), RafflesSplit::HeldOut);
        let units: Vec<String> = DoublePipeOutput::channels()
            .into_iter()
            .map(|c| c.unit)
            .collect();
        assert_eq!(units[0], "m^2 kg s^-3 K^-1");
        assert_eq!(units[3], "K");
        assert_eq!(Theta::channels()[2].unit, "kg^-1 s^3 K");
    }

    /// θ nominal lies in the provisional parameter space, and a failing x
    /// (k_wall not a TUAS wall level) comes back as `ModelEvaluation`.
    #[test]
    fn the_parameter_space_holds_nominal_theta() {
        let space = TuasOneD.parameter_space();
        assert!(space.contains(&Theta::nominal()));
        assert_eq!(space.channels().len(), 5);
        let bad = DoublePipeInput::si(30.0, 15.0, 7.0, 0.2);
        let e = TuasOneD.evaluate(&bad, &Theta::nominal()).unwrap_err();
        assert!(matches!(e, RafflesError::ModelEvaluation { .. }), "{e}");
    }

    /// **Methodology.** `CorrectedModel { TuasOneD, θ nominal, no δ }`
    /// against `one_d::evaluate(&x, &Theta::nominal())` at the 8 held-out
    /// points of the committed dataset, all nine output channels compared
    /// with `to_bits`. Run on 4 threads (16 TUAS marches, ≈ 0.4–30 s each).
    ///
    /// **Prediction** (V&V record P5, written before the run): bit-identical.
    /// **Result (2026-11-04):** bit-identical on all 8 points × 9 channels
    /// (whole test 32 s on 4 threads, with the other three tests).
    #[test]
    fn corrected_model_at_nominal_theta_is_one_d_bit_for_bit() {
        let d = committed_dataset().unwrap();
        let held: Vec<DoublePipeInput> = (0..d.len())
            .filter_map(|i| d.row(i).ok())
            .filter(|r| r.2 == RafflesSplit::HeldOut)
            .map(|r| r.0)
            .collect();
        assert_eq!(held.len(), 8);
        let model = CorrectedModel::new(TuasOneD, ThetaEstimate::Point(Theta::nominal()), None);
        let results: Vec<(Vec<u64>, Vec<u64>)> = std::thread::scope(|s| {
            let handles: Vec<_> = held
                .chunks(2)
                .map(|chunk| {
                    let model = &model;
                    s.spawn(move || {
                        chunk
                            .iter()
                            .map(|x| {
                                let via = model.predict(x).unwrap();
                                assert!(via.std_dev.is_none());
                                let direct = one_d::evaluate(x, &Theta::nominal());
                                let bits = |o: &DoublePipeOutput| {
                                    Quantities::to_si(o)
                                        .iter()
                                        .map(|v| v.to_bits())
                                        .collect::<Vec<u64>>()
                                };
                                (bits(&via.mean), bits(&direct))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        assert_eq!(results.len(), 8);
        for (i, (a, b)) in results.iter().enumerate() {
            assert_eq!(a, b, "held-out point {i}");
        }
    }

    /// **Methodology.** Polynomial surrogates of the nine CFD outputs as
    /// functions of the raw x = (Re_hot, Re_cold, k_wall, L), fitted through
    /// `Surrogate::fit` to the 36 training rows of the committed dataset and
    /// validated by `ValidationReport::of` on the 8 held-out rows, with LOO on
    /// the training rows. Two settings, fixed before the run and not revisited
    /// (V&V record P6): degree 1 ridge 0, and degree 2 ridge 1e-8. No tuning:
    /// the numbers are reported, and the only gates are that both fits exist
    /// and that every reported error is finite.
    ///
    /// **Prediction** (V&V record P6): UA 10–30 % relative held-out RMSE at
    /// both degrees (fit linear in k_wall; six held-out points at k_wall 10);
    /// Δp 20–40 % at degree 1, under 10 % at degree 2; outlet T 1–4 K / 0.5–3 K;
    /// LOO RMSE above held-out RMSE for most outputs at degree 2.
    /// **Result (2026-11-04)**, relative = held-out RMSE / mean |y| held out:
    ///
    /// | channel | degree 1 RMSE (rel.) | degree 1 LOO | degree 2 RMSE (rel.) | degree 2 LOO |
    /// |---|---|---|---|---|
    /// | UA \[W/K\] | 0.1328 (11.1 %) | 0.0859 | 0.5988 (50.0 %) | 0.0525 |
    /// | Q \[W\] | 7.349 (25.1 %) | 7.916 | 16.66 (56.8 %) | 4.969 |
    /// | ε | 0.0880 (13.7 %) | 0.1668 | 0.2525 (39.4 %) | 0.0909 |
    /// | T_hot,out \[K\] | 3.503 | 5.629 | 126.6 | 2.838 |
    /// | T_cold,out \[K\] | 3.489 | 7.027 | 125.5 | 2.399 |
    /// | Δp_hot \[Pa\] | 0.0311 (11.3 %) | 0.1323 | 0.1370 (49.8 %) | 7.1e-6 |
    /// | Δp_cold \[Pa\] | 0.0430 (12.8 %) | 0.1806 | 0.1779 (53.0 %) | 3.3e-7 |
    /// | Nu_hot | 0.2694 (5.2 %) | 0.3985 | 1.999 (38.3 %) | 0.1809 |
    /// | Nu_cold | 0.1512 (2.3 %) | 0.3058 | 2.580 (39.0 %) | 0.1615 |
    ///
    /// **Degree 2 fails on the held-out points, and the cause is the basis,
    /// not the data.** On the 2-level k_wall and L factors, scaled to ±1,
    /// k² and L² equal 1 at every training row, so they are the constant
    /// column again; the ridge picks the minimum-norm split of the constant
    /// across the three, and at an interior point (k² ≈ 0.67 at k_wall 10,
    /// L² = 0 at L 0.2) a share of the constant disappears, hence 126 K off
    /// on a 300–350 K outlet. LOO cannot see this (every LOO point is a grid
    /// node, where the split is harmless), which is why degree 2's LOO looks
    /// excellent: **an in-grid LOO is not evidence for interior accuracy on
    /// this design.** The prediction (Δp under 10 % at degree 2) missed
    /// badly; degree 1 met or beat most of its predictions (record P6).
    /// Nothing was tuned in response.
    #[test]
    fn polynomial_surrogate_of_the_cfd_held_out_report() {
        let d = committed_dataset().unwrap();
        let (_, hy) = d.held_out_si();
        let names: Vec<String> = DoublePipeOutput::channels()
            .into_iter()
            .map(|c| format!("{} [{}]", c.name, c.unit))
            .collect();
        for (degree, ridge) in [(1usize, 0.0), (2, 1e-8)] {
            let spec = SurrogateSpec::Polynomial { degree, ridge };
            let s = Surrogate::fit(&spec, &d).unwrap();
            let r = ValidationReport::of(&s, &spec, &d, true).unwrap();
            assert_eq!(r.counts, (36, 8));
            let rmse = Quantities::to_si(&r.held_out_rmse.unwrap());
            let max = Quantities::to_si(&r.held_out_max_error.unwrap());
            let loo = Quantities::to_si(&r.loo_rmse.unwrap());
            println!("degree {degree}, ridge {ridge:e}: channel | held-out RMSE | relative | max |error| | LOO RMSE");
            for k in 0..names.len() {
                let mean = hy.iter().map(|y| y[k].abs()).sum::<f64>() / hy.len() as f64;
                println!(
                    "  {} | {:.4e} | {:.2} % | {:.4e} | {:.4e}",
                    names[k],
                    rmse[k],
                    100.0 * rmse[k] / mean,
                    max[k],
                    loo[k]
                );
                assert!(
                    rmse[k].is_finite() && max[k].is_finite() && loo[k].is_finite(),
                    "{}",
                    names[k]
                );
            }
        }
    }
}
