use super::*;
use crate::msre::circulation::{CirculatingKinetics, CirculationModel};

/// **Verification: the simulator's line model against Moltres's precursor
/// drift, live** (maintainer, 2026-11-14).
///
/// Methodology: the circulation loss at full flow from
/// [`CirculationModel::LineTm1626`] (the MSRE simulator's default kinetics;
/// time-domain plug-flow ring, dt = 0.01 s, generation time 2.4e-4 s)
/// against [`line_loss`] (Moltres's finite-volume steady drift, first-order
/// upwind, on 2520 and 10 080 cells), both on TM-1626 Table 1 and Fig. 4.
/// Plus two limits: stationary fuel loses nothing, and a negative flow is
/// refused.
///
/// Prediction (written before the first run, 2026-11-14): gh:#1130 records
/// Moltres converging to the exact line model at first order (4.4e-8 δk/k
/// at 10 080 cells, so ~2e-7 at 2520), and gh:#1330 records the line model
/// at −1.7e-6 δk/k from exact. So the two differ by about 2e-6 δk/k:
/// **|difference| < 5e-6 δk/k** at both resolutions, and the stationary loss
/// below 1e-12.
///
/// Result (2026-11-14, release build): line model **0.229140 %**; Moltres
/// **0.229290 %** at 2520 cells and **0.229303 %** at 10 080 (gh:#1130's
/// exact line model: 0.2293 %). Differences **−1.49e-6** and **−1.63e-6
/// δk/k**, inside the predicted 5e-6: the simulator's kinetics and Moltres's
/// spatial drift agree on the circulation loss to 0.7 % of it. Moltres
/// moves towards the exact value as the mesh refines (first-order upwind);
/// the remaining −1.6e-6 is the line model's own 0.01 s cell discretisation
/// (gh:#1330 records −1.7e-6 from exact). Stationary fuel: loss below
/// 1e-12. Both sit +8 % above the measured 0.212 ± 0.004 % (TM-1626), as
/// recorded on gh:#1130.
#[test]
fn the_line_model_agrees_with_moltres() {
    let model = CirculatingKinetics::new(CirculationModel::LineTm1626, 2.4e-4, 0.01).expect("line model").circulation_loss();
    let m2520 = line_loss(2520, 1.0).expect("moltres 2520");
    let m10080 = line_loss(10_080, 1.0).expect("moltres 10080");
    println!(
        "circulation loss, % dk/k: line model {:.6}, moltres 2520 {:.6}, moltres 10080 {:.6}; diff {:.2e} / {:.2e} dk/k",
        100.0 * model,
        100.0 * m2520,
        100.0 * m10080,
        model - m2520,
        model - m10080
    );
    assert!((model - m2520).abs() < 5e-6, "{model} vs {m2520}");
    assert!((model - m10080).abs() < 5e-6, "{model} vs {m10080}");
    let still = line_loss(2520, 0.0).expect("stationary");
    assert!(still.abs() < 1e-12, "stationary fuel lost {still:e}");
    assert!(line_loss(2520, -1.0).is_err());
    assert_eq!(tm1626_families().len(), 6);
    assert!((tm1626_loop_time() - 25.2).abs() < 1e-12);
}

/// **The plant-coupled drift ([`LoopDrift`]), gh:#1575.**
///
/// Methodology: [`LoopDrift`] on [`LOOP_DRIFT_CELLS`] cells, from the
/// steady loop at full flow and 8 MW. (1) Held at full flow for 30 s in
/// 0.1 s advances: the loss stays at [`line_loss`] on the same mesh.
/// (2) The fuel pump stops (flow 0) and the plant holds 8 MW: the loss falls
/// towards zero as the precursors stop leaving the core.
///
/// Prediction (posted on gh:#1575 before running, 2026-11-14): (1) within
/// 1e-9 δk/k; (2) below 5 % of its starting value after 600 s, the longest
/// group (λ = 0.0124 /s) setting the tail.
///
/// Result (2026-11-14, release): (1) steady 0.229218 %, worst drift over
/// 30 s **5.1e-15 δk/k**. (2) Loss every 60 s after the pump stops, %:
/// 0.0195, 0.0050, 0.0017, 0.00071, 0.00032, 0.00015, 7.1e-5, 3.4e-5,
/// 1.6e-5, **8.0e-6**: monotone, and **0.0035 %** of the starting value at
/// 600 s. Both inside the prediction. The fall is far faster than the
/// longest group alone suggests (e^(-0.0124 × 600) = 6e-4 of its own
/// share): that group carries a small share of β, and the fast groups
/// refill the core within seconds.
#[test]
fn the_loop_drift_holds_steady_and_relaxes_when_the_pump_stops() {
    let mut d = LoopDrift::new(LOOP_DRIFT_CELLS, 1.0, 8.0).expect("drift");
    let steady = line_loss(LOOP_DRIFT_CELLS, 1.0).expect("line loss");
    let l0 = d.loss().expect("loss");
    for _ in 0..300 {
        d.advance(0.1, 1.0, 8.0).expect("advance");
    }
    let l30 = d.loss().expect("loss");
    let worst_hold = (l0 - steady).abs().max((l30 - steady).abs());
    // Pump off.
    let mut trace = Vec::new();
    for k in 0..600 {
        d.advance(1.0, 0.0, 8.0).expect("advance");
        if k % 60 == 59 {
            trace.push(d.loss().expect("loss"));
        }
    }
    println!(
        "loop drift: steady {:.6} %, held 30 s {:.6} % (worst {:.1e}); pump off, loss every 60 s, %: {:?}",
        100.0 * steady,
        100.0 * l30,
        worst_hold,
        trace.iter().map(|x| (1e6 * 100.0 * x).round() / 1e6).collect::<Vec<_>>()
    );
    assert!(worst_hold < 1e-9, "{worst_hold:e}");
    assert!(trace.windows(2).all(|w| w[1] <= w[0] + 1e-12), "not monotone: {trace:?}");
    assert!(*trace.last().unwrap() < 0.05 * steady, "{trace:?}");
    assert!((d.time() - 630.0).abs() < 1e-9);
}

/// The bookkeeping: binned emission keeps the mean, zero power has no loss,
/// bad inputs are refused, a long advance is capped at
/// [`MAX_STEPS_PER_ADVANCE`] steps and still conserves the time, and the
/// region starts are TM-1626's.
#[test]
fn the_loop_drift_bookkeeping() {
    let mut d = LoopDrift::new(LOOP_DRIFT_CELLS, 1.0, 8.0).expect("drift");
    let e = d.emission();
    let b = d.emission_binned(126);
    assert_eq!(e.len(), LOOP_DRIFT_CELLS);
    assert_eq!(b.len(), 126);
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    assert!((mean(&e) - mean(&b)).abs() < 1e-12 * mean(&e).abs().max(1.0));
    d.advance(10.0, 1.0, 8.0).expect("long advance");
    assert!((d.time() - 10.0).abs() < 1e-12);
    assert!(d.advance(1.0, -0.1, 8.0).is_err());
    assert!(d.advance(1.0, 1.0, f64::NAN).is_err());
    assert!(d.advance(-1.0, 1.0, 8.0).is_err());
    d.advance(0.0, 1.0, 8.0).expect("zero advance");
    let z = LoopDrift::new(LOOP_DRIFT_CELLS, 1.0, 0.0).expect("zero power");
    assert_eq!(z.loss(), None);
    let r = LoopDrift::region_starts();
    assert_eq!(r[0], 0.0);
    assert!((r[1] - 3.8 / 25.2).abs() < 1e-12 && (r[3] - 17.1 / 25.2).abs() < 1e-12);
}
