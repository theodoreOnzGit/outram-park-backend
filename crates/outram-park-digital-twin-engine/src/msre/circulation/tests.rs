use super::*;
use teh_o_prke::zero_power_prke::circulating::CirculatingPointKinetics;
use uom::si::f64::{Frequency, Ratio, Time, VolumetricNumberRate};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

const DT: f64 = 0.01;
const LAMBDA_GEN: f64 = 2.4e-4;

/// The measured loss, ORNL-TM-1626 (% δk/k), and its uncertainty.
const MEASURED: (f64, f64) = (0.212, 0.004);

/// **Differential test, gh:#1332: the variable-flow kinetics at full flow
/// against `teh_o_prke`'s `CirculatingPointKinetics` (MSRE T-C1).**
///
/// Methodology: both built with TM-1070's six groups, τ_c 8.46 s, τ_L
/// 16.73 s, Λ 2.4e-4 s, dt 0.01 s, and driven for 300 s at the circulation
/// loss plus a rod sinusoid of 1e-4 at 0.05 rad/s. Predicted (gh:#1332):
/// the same backward-Euler scheme, so equal to round-off, ≤ 1e-12 relative
/// in `n` at every step. The schemes differ only in how the loop's decay
/// factor is formed (`e^{−λ (k_now − k_left) dt}` here against
/// `e^{−λ τ_L}`).
///
/// Result (2026-11-10): **bit-identical** in `n` at all 30 000 steps
/// (worst relative difference 0); the circulation losses differ by 8.7e-19
/// (0.24670411828487 %), the analytic formula against the scheme's own
/// equilibrium.
#[test]
fn the_well_stirred_model_is_teh_o_prkes_at_full_flow() {
    let mut ours = CirculatingKinetics::new(CirculationModel::WellStirredTm1070, LAMBDA_GEN, DT).expect("ours");
    let mut theirs = CirculatingPointKinetics::with_groups(
        BETA.iter().map(|&b| Ratio::new::<ratio>(b)).collect(),
        LAMBDA.iter().map(|&l| Frequency::new::<hertz>(l)).collect(),
        Time::new::<second>(TM1070_CORE_TRANSIT_S),
        Time::new::<second>(TM1070_LOOP_TRANSIT_S),
        Time::new::<second>(LAMBDA_GEN),
        Time::new::<second>(DT),
    )
    .expect("theirs");
    let loss_theirs = theirs.circulation_reactivity_loss().get::<ratio>();
    let loss_ours = ours.circulation_loss();
    println!("loss ours {loss_ours:.17e}, teh-o-prke {loss_theirs:.17e}, diff {:e}", loss_ours - loss_theirs);
    assert!((loss_ours - loss_theirs).abs() < 1e-15);
    let mut worst = 0.0_f64;
    for k in 0..30_000 {
        let rho = loss_theirs + 1.0e-4 * (0.05 * k as f64 * DT).r_sin();
        ours.step(rho, 1.0, 0.0).expect("step");
        theirs
            .step_implicit(Ratio::new::<ratio>(rho), VolumetricNumberRate::new::<per_cubic_meter_second>(0.0))
            .expect("step");
        let n_theirs = theirs.neutron_density().get::<per_cubic_meter>();
        worst = worst.max((ours.neutron_density() / n_theirs - 1.0).abs());
    }
    println!("worst relative n difference over 300 s: {worst:e}");
    assert!(worst <= 1e-12);
}

/// The exact steady loss of TM-1626's line model, plug flow everywhere:
/// `dc/dt = β_i S(t) − λ_i c` integrated exactly around the loop with `S`
/// linear on each of `m` steps, emission and normalisation by the
/// trapezoid rule. Ported from `exact_loss` (plug-flow branch) in
/// `outram-park-fork-moltres/tests/msre_n4_precursor_drift_tm1626.rs`
/// (gh:#1130), which records 0.2293 % for it.
fn exact_line_loss(m: usize) -> f64 {
    let t_loop: f64 = TM1626_RESIDENCE_S.iter().sum();
    let dt = t_loop / m as f64;
    let samples: Vec<(f64, f64)> = (0..=m).map(|j| tm1626_path_at(j as f64 * dt)).collect();
    let denom: f64 = (0..m).map(|j| 0.5 * (samples[j].0 * samples[j].1 + samples[j + 1].0 * samples[j + 1].1) * dt).sum();
    let mut delayed = 0.0;
    for &(beta, lambda) in &TM1626_TABLE1 {
        let e = (-lambda * dt).r_exp();
        let g1 = 1.0 / lambda - (1.0 - e) / (lambda * lambda * dt);
        let g0 = (1.0 - e) / lambda - g1;
        let run = |c0: f64, record: bool| -> (f64, f64) {
            let mut c = c0;
            let mut acc = 0.0;
            for j in 0..m {
                let c1 = e * c + beta * (samples[j].0 * g0 + samples[j + 1].0 * g1);
                if record {
                    acc += 0.5 * lambda * (c * samples[j].1 + c1 * samples[j + 1].1) * dt;
                }
                c = c1;
            }
            (c, acc)
        };
        let b = run(0.0, false).0;
        let a = run(1.0, false).0 - b;
        delayed += run(b / (1.0 - a), true).1;
    }
    let beta: f64 = TM1626_TABLE1.iter().map(|x| x.0).sum();
    beta - delayed / denom
}

/// **Verification, gh:#1330: the time-domain line model's steady loss
/// against the exact line model on the same shapes.**
///
/// Methodology: [`CirculationModel::LineTm1626`] at dt = 0.01 s (2520
/// cells, source and importance at cell centres) against
/// [`exact_line_loss`] at 25 200 and 50 400 steps; then 300 s at full flow
/// held at the loss, checking the equilibrium. Predicted (gh:#1330, before
/// running): equal to the exact 0.2293 % within 0.0005 % δk/k; steady to
/// 1e-9.
///
/// Result (2026-11-10): exact **0.229308 %** (25 200 steps; 0.229307 % at
/// 50 400: the reference converges at second order over Fig. 4's kinks, so
/// its self-check is 1e-7, as in the moltres test); time-domain **0.229140
/// %**, difference **−1.7e-6 δk/k (−1.7e-4 % δk/k)**, inside the predicted
/// 0.0005 %. Steady: worst `|n − 1|` 3.9e-13 over 300 s. Against the
/// measurement, 0.212 ± 0.004 %: **+0.0171 %, +8.1 %**, as #1130 recorded for
/// the same shapes (0.2293 %).
#[test]
fn the_line_model_reproduces_the_exact_line_model() {
    let exact = exact_line_loss(25_200);
    let exact_fine = exact_line_loss(50_400);
    let mut k = CirculatingKinetics::new(CirculationModel::LineTm1626, LAMBDA_GEN, DT).expect("line");
    let loss = k.circulation_loss();
    println!(
        "line model: time-domain {:.6} %, exact {:.6} % (fine {:.6} %), diff {:.2e} % dk/k; vs measured {:+.4} % ({:+.1} %)",
        100.0 * loss,
        100.0 * exact,
        100.0 * exact_fine,
        100.0 * (loss - exact),
        100.0 * loss - MEASURED.0,
        100.0 * (100.0 * loss / MEASURED.0 - 1.0)
    );
    // The exact reference converges at second order (trapezoid over Fig. 4's
    // kinks): its own 25 200- against 50 400-step change is ~1e-8.
    assert!((exact - exact_fine).abs() < 1e-7);
    assert!((loss - exact).abs() < 5e-6);
    let mut worst = 0.0_f64;
    for _ in 0..30_000 {
        k.step(loss, 1.0, 0.0).expect("step");
        worst = worst.max((k.neutron_density() - 1.0).abs());
    }
    println!("line model steady 300 s: worst |n - 1| = {worst:e}");
    assert!(worst <= 1e-9);
}

/// **Stopping the flow gives back the circulation loss (both models).**
///
/// Methodology: from the circulating steady state, flow 0, the reactor held
/// critical by setting the reactivity each step to
/// [`CirculatingKinetics::holding_reactivity`] (an exact servo). After
/// 1500 s (18.6 lifetimes of the longest group) the critical reactivity
/// must be that of a stationary core, zero, within 1e-6. Harness check of
/// the zero-flow limit, not V&V.
///
/// Result (2026-11-10): critical reactivity left after 1500 s, well-stirred
/// 1.2e-12, line 1.1e-12; `n` held to 1.1e-16.
#[test]
fn stopping_the_flow_gives_back_the_loss() {
    for model in [CirculationModel::WellStirredTm1070, CirculationModel::LineTm1626] {
        let mut k = CirculatingKinetics::new(model, LAMBDA_GEN, DT).expect("kinetics");
        let mut worst_n = 0.0_f64;
        for _ in 0..150_000 {
            let rho = k.holding_reactivity(0.0, 1.0);
            k.step(rho, 0.0, 0.0).expect("step");
            worst_n = worst_n.max((k.neutron_density() - 1.0).abs());
        }
        let left = k.critical_reactivity();
        println!("{model:?}: critical reactivity after 1500 s stationary {left:e}; worst |n-1| {worst_n:e}");
        assert!(left.abs() < 1e-6);
        assert!(worst_n < 1e-6);
        assert_eq!(k.flow_fraction(), 0.0);
    }
    let mut k = CirculatingKinetics::new(CirculationModel::WellStirredTm1070, LAMBDA_GEN, DT).expect("kinetics");
    assert!(k.step(0.0, -0.1, 0.0).is_err());
    assert!(k.step(f64::NAN, 1.0, 0.0).is_err());
    assert!(CirculatingKinetics::new(CirculationModel::LineTm1626, LAMBDA_GEN, 0.003).is_err());
    assert!(CirculatingKinetics::new(CirculationModel::WellStirredTm1070, 0.0, DT).is_err());
}

/// ENDF/B-VIII.0 U-235 (`reference-data/endf/n-092_U_235-ENDF8.0.endf`,
/// MAT 9228): decay constants from MF1/MT455 (line 1121: 1.3336e-2,
/// 3.2739e-2, 1.2078e-1, 3.0278e-1, 8.4949e-1, 2.853 /s) and the
/// energy-independent group abundances from MF5/MT455 (lines 20225 ff.:
/// 0.03500755, 0.1806982, 0.1725102, 0.3867821, 0.1585752, 0.0664266).
const ENDF8_U235: [(f64, f64); 6] = [
    (0.03500755, 1.3336e-2),
    (0.1806982, 3.2739e-2),
    (0.1725102, 1.2078e-1),
    (0.3867821, 3.0278e-1),
    (0.1585752, 8.4949e-1),
    (0.0664266, 2.853),
];
/// ENDF/B-VIII.0 U-235 thermal ν_d (MF1/MT455, 1.585e-2) over ν (MF1/MT452
/// at 1e-5 to 0.2 eV, 2.42985).
const ENDF8_U235_BETA: f64 = 1.585e-2 / 2.42985;

/// **gh:#1330 residual investigation: data-set and transit ablations of
/// the well-stirred loss.**
///
/// Methodology: the analytic well-stirred loss [`well_stirred_loss`] with
/// one input changed at a time from the model's (TM-1070 App. B data, τ_c
/// 8.46 s, τ_L 16.73 s): (1a) TM-1626 Table 1's data; (1b) ENDF/B-VIII.0
/// U-235's group shape, total held at 0.006661, and (1b') at ENDF's own
/// β = ν_d/ν; (2) TM-1626's transits, 9.4 s core and 15.8 s loop. Also
/// TM-1070 App. B p. 69's own circulating effective β (0.000277, 0.000718,
/// 0.001698, 0.000499, 0.000373, 0.000052), as ORNL's 1965 value.
/// Predictions (posted on gh:#1330 before running): (1a) |Δ| < 0.001 %;
/// (1b) −3 to −8 %, and a further ≈ −2.5 % for (1b').
///
/// Results (2026-11-10), % δk/k, against the model's 0.246704:
///
/// | case | loss | Δ |
/// |---|---|---|
/// | (1a) TM-1626 Table 1 data | 0.246576 | −0.000128 (−0.05 %) |
/// | (1b) ENDF/B-VIII.0 shape, β 0.006661 | 0.226990 | −0.019714 (**−8.0 %**) |
/// | (1b') ENDF/B-VIII.0, own β 0.006523 | 0.222289 | −0.024415 (−9.9 %) |
/// | (2) TM-1626 transits 9.4 / 15.8 s | 0.230211 | −0.016494 (−6.7 %) |
/// | TM-1070 App. B's own effective β | 0.304400 | +0.057696 |
///
/// (1a) as predicted (|Δ| < 0.001 %): not a cause. (1b) −8.0 %, at the edge
/// of the predicted −3 to −8 %; (1b') a further −1.9 % against the predicted
/// ≈ −2.5 %. ENDF/B-VIII.0 puts more of β in the short-lived groups (5 and
/// 6: 22.5 % of β against TM-1070's 15.7 %), which decay before leaving the
/// core. It is not adopted: TM-1070's set is the one ORNL's T-1 model and
/// TM-1626's reactivity scale (its rod calibration converts periods with
/// these groups and β = 0.00666) were built on, so changing it changes the
/// instrument as well as the model.
#[test]
fn the_data_set_and_transit_ablations() {
    let tm1070: Vec<(f64, f64)> = BETA.iter().cloned().zip(LAMBDA.iter().cloned()).collect();
    let base = well_stirred_loss(&tm1070, TM1070_CORE_TRANSIT_S, TM1070_LOOP_TRANSIT_S);
    let total: f64 = BETA.iter().sum();
    let endf_shape: Vec<(f64, f64)> = ENDF8_U235.iter().map(|&(a, l)| (a * total, l)).collect();
    let endf_own: Vec<(f64, f64)> = ENDF8_U235.iter().map(|&(a, l)| (a * ENDF8_U235_BETA, l)).collect();
    let cases = [
        ("(1a) TM-1626 data", well_stirred_loss(&TM1626_TABLE1, TM1070_CORE_TRANSIT_S, TM1070_LOOP_TRANSIT_S)),
        ("(1b) ENDF/B-VIII.0 shape, beta 0.006661", well_stirred_loss(&endf_shape, TM1070_CORE_TRANSIT_S, TM1070_LOOP_TRANSIT_S)),
        ("(1b') ENDF/B-VIII.0 own beta", well_stirred_loss(&endf_own, TM1070_CORE_TRANSIT_S, TM1070_LOOP_TRANSIT_S)),
        ("(2) TM-1626 transits", well_stirred_loss(&tm1070, 9.4, 15.8)),
    ];
    println!("model (TM-1070): {:.6} %", 100.0 * base);
    for (name, v) in cases {
        println!("{name}: {:.6} %, delta {:+.6} % ({:+.2} %)", 100.0 * v, 100.0 * (v - base), 100.0 * (v / base - 1.0));
    }
    let eff_1965 = [0.000277, 0.000718, 0.001698, 0.000499, 0.000373, 0.000052];
    let ornl_1965 = total - eff_1965.iter().sum::<f64>();
    println!("TM-1070 App. B effective beta: {:.6} % (sum {:.6})", 100.0 * ornl_1965, eff_1965.iter().sum::<f64>());
    println!("ENDF/B-VIII.0 U-235 beta = {ENDF8_U235_BETA:.6}; abundances sum {:.7}", ENDF8_U235.iter().map(|x| x.0).sum::<f64>());
    let k = CirculatingKinetics::new(CirculationModel::WellStirredTm1070, LAMBDA_GEN, DT).expect("kinetics");
    assert!((k.circulation_loss() - base).abs() < 1e-15);
    assert!((100.0 * base - 0.2467041).abs() < 1e-7);
    assert!((cases[0].1 - base).abs() < 1e-5);
    assert!(cases[1].1 < base && cases[2].1 < cases[1].1);
    assert!((100.0 * ornl_1965 - 0.3044).abs() < 1e-4);
    assert!((total - 0.006661).abs() < 1e-12);
}

/// The TM-1626 path and shape are transcribed as cited.
#[test]
fn the_tm1626_path_is_the_cited_one() {
    assert_eq!(tm1626_shape_at(90.0), (0.990, 0.990));
    assert_eq!(tm1626_shape_at(-50.0), (0.015, 0.045));
    assert_eq!(tm1626_shape_at(500.0), (0.008, 0.070));
    let (p, w) = tm1626_shape_at(15.0);
    assert!((p - 0.415).abs() < 1e-12 && (w - 0.435).abs() < 1e-12);
    // Mid lower plenum (z = -11), the core mid-height (z = 84), outside.
    assert_eq!(tm1626_path_at(1.9), tm1626_shape_at(-11.0));
    assert_eq!(tm1626_path_at(3.8 + 4.7), tm1626_shape_at(84.0));
    assert_eq!(tm1626_path_at(20.0), (0.0, 0.0));
    let beta: f64 = TM1626_TABLE1.iter().map(|x| x.0).sum();
    assert!((beta - 0.006661).abs() < 1e-12);
    assert!((TM1626_RESIDENCE_S.iter().sum::<f64>() - 25.2).abs() < 1e-12);
}
