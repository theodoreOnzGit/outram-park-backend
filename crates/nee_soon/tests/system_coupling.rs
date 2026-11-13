//! # V&V: system-code / point-kinetics coupling interface (gh:#1084)
//!
//! A three-node lumped loop (per node: a fuel and a coolant temperature,
//! backward Euler) is coupled through `LoopKineticsCoupling` to six-group
//! U-235 point kinetics with Doppler feedback evaluated by GeN-Foam's
//! `ReactivityFeedback`. The **monolithic** reference solves the same
//! backward-Euler system for all 13 unknowns at once with petir's JFNK.
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. The feedback the port computes equals the hand
//!    `−α (T̄_f − T_ref)` to round-off.
//! 2. With Picard to 1e-12, the coupled transient reproduces the monolithic
//!    one: power and fuel temperatures agree within 1e-8 relative at every
//!    step of a 2 s, +0.1 β step transient at Δt = 0.01 s.
//! 3. Loose coupling (one pass) differs from the monolithic solution by a
//!    splitting error that halves with Δt (first order; ratio in [1.7, 2.3]).

use nee_soon::system_coupling::*;
use nee_soon::NeeSoon;
use outram_foam_appbuilder_lib::genfoam::multi_region::{DopplerLaw, ImportanceWeight, ReactivityFeedback};
use petir::newton_krylov::{jfnk, JfnkConfig};
use teh_o_prke::zero_power_prke::n_group_prke::PointKinetics;
use teh_o_prke::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

const N: usize = 3;
const F: [f64; N] = [0.3, 0.4, 0.3];
const P0: f64 = 3.0e6;
const CF: f64 = 5.0e4; // fuel heat capacity per node, J/K
const HA: f64 = 2.0e4; // fuel-coolant conductance, W/K
const CC: f64 = 2.0e4; // coolant heat capacity per node, J/K
const WC: f64 = 1.0e5; // coolant capacity rate, W/K
const T_IN: f64 = 550.0;
const ALPHA: f64 = 3.0e-5;
const LAMBDA: f64 = 1.0e-4;

#[derive(Clone, Debug)]
struct LumpedLoop {
    tf: [f64; N],
    tc: [f64; N],
}

impl LumpedLoop {
    fn steady(p: f64) -> Self {
        let mut s = Self { tf: [0.0; N], tc: [0.0; N] };
        for i in 0..N {
            s.tc[i] = T_IN + F[i] * p / WC;
            s.tf[i] = s.tc[i] + F[i] * p / HA;
        }
        s
    }
}

impl SystemCodeSide for LumpedLoop {
    fn node_field(&self, name: &str) -> Result<Vec<f64>, SystemCouplingError> {
        match name {
            "TFuel" => Ok(self.tf.to_vec()),
            _ => Err(SystemCouplingError::SystemCode(format!("no field {name}"))),
        }
    }
    fn advance(&mut self, dt: Time, q: &[f64]) -> Result<(), SystemCouplingError> {
        let dt = dt.get::<second>();
        for i in 0..N {
            // Backward Euler on the 2x2 node system, solved exactly.
            let (a11, a12, b1) = (CF / dt + HA, -HA, CF / dt * self.tf[i] + q[i]);
            let (a21, a22, b2) = (-HA, CC / dt + HA + WC, CC / dt * self.tc[i] + WC * T_IN);
            let det = a11 * a22 - a12 * a21;
            self.tf[i] = (b1 * a22 - a12 * b2) / det;
            self.tc[i] = (a11 * b2 - a21 * b1) / det;
        }
        Ok(())
    }
}

fn t_ref() -> f64 {
    LumpedLoop::steady(P0).tf.iter().sum::<f64>() / N as f64
}

fn coupling() -> LoopKineticsCoupling {
    let fb = ReactivityFeedback::new()
        .with_weight(ImportanceWeight::Volume)
        .with_doppler("TFuel", TemperatureCoefficient::new::<per_kelvin>(ALPHA), ThermodynamicTemperature::new::<kelvin>(t_ref()), DopplerLaw::Linear);
    NeeSoon::default()
        .new_loop_kinetics_coupling(
            PointKinetics::from_nuclide(FissioningNuclideType::U235),
            Time::new::<second>(LAMBDA),
            Power::new::<watt>(P0),
            &[1.0; N],
            &F,
            fb,
        )
        .unwrap()
}

/// The monolithic backward-Euler step: unknowns `[n̂, Ĉ_1..6, T_f1..3,
/// T_c1..3]`, `n̂ = n/n_0`, solved by JFNK to 1e-13.
fn monolithic_step(x0: &[f64], dt: f64, rho_ext: f64, lam: &[f64], beta: &[f64]) -> Vec<f64> {
    let bt: f64 = beta.iter().sum();
    let tr = t_ref();
    let f = |x: &[f64], r: &mut [f64]| {
        let n = x[0];
        let tf_mean = (x[7] + x[8] + x[9]) / 3.0;
        let rho = rho_ext - ALPHA * (tf_mean - tr);
        let mut src = 0.0;
        for i in 0..6 {
            src += lam[i] * x[1 + i];
            r[1 + i] = (x[1 + i] - x0[1 + i]) / dt - (beta[i] / LAMBDA * n - lam[i] * x[1 + i]);
        }
        r[0] = ((n - x0[0]) / dt - ((rho - bt) / LAMBDA * n + src)) * LAMBDA;
        for i in 0..N {
            let (tf, tc) = (x[7 + i], x[10 + i]);
            r[7 + i] = (CF * (tf - x0[7 + i]) / dt - (F[i] * P0 * n - HA * (tf - tc))) / P0;
            r[10 + i] = (CC * (tc - x0[10 + i]) / dt - (HA * (tf - tc) - WC * (tc - T_IN))) / P0;
        }
    };
    let mut x = x0.to_vec();
    let cfg = JfnkConfig { abs_tol: 1e-13, rel_tol: 1e-14, step_tol: 1e-15, ..JfnkConfig::default() };
    jfnk(&f, &mut x, &cfg).unwrap();
    x
}

/// Run both to 2 s; returns (worst relative power difference, worst relative
/// fuel-temperature difference, final power) of coupled vs monolithic.
fn compare(dt: f64, control: PicardControl) -> (f64, f64, f64) {
    let mut cpl = coupling();
    let rho_ext = 0.1 * cpl.kinetics().total_delayed_fraction().get::<ratio>();
    cpl.set_external_reactivity(Ratio::new::<ratio>(rho_ext));
    let mut side = LumpedLoop::steady(P0);
    let lam: Vec<f64> = cpl.kinetics().decay_constants().iter().map(|l| l.get::<hertz>()).collect();
    let beta: Vec<f64> = cpl.kinetics().delayed_fractions().iter().map(|b| b.get::<ratio>()).collect();
    let mut x: Vec<f64> = vec![1.0];
    for i in 0..6 {
        x.push(beta[i] / (LAMBDA * lam[i]));
    }
    x.extend_from_slice(&side.tf);
    x.extend_from_slice(&side.tc);
    let steps = (2.0 / dt).round() as usize;
    let (mut wp, mut wt) = (0.0_f64, 0.0_f64);
    for _ in 0..steps {
        cpl.coupled_step(&mut side, Time::new::<second>(dt), control).unwrap();
        x = monolithic_step(&x, dt, rho_ext, &lam, &beta);
        let p = cpl.power().get::<watt>();
        wp = wp.max((p / (P0 * x[0]) - 1.0).abs());
        for i in 0..N {
            wt = wt.max((side.tf[i] / x[7 + i] - 1.0).abs());
        }
    }
    (wp, wt, cpl.power().get::<watt>())
}

/// **The port's feedback is the hand feedback.** Nodes 10 K above their
/// steady fuel temperatures: `ρ_fb = −α·10 K = −3e-4`. **Result
/// (2026-11-01):** −3e-4 to round-off (zero at the reference state). Held.
#[test]
fn the_feedback_through_the_port_is_the_hand_doppler() {
    let mut cpl = coupling();
    let mut s = LumpedLoop::steady(P0);
    assert!(cpl.reactivity_of(&s).unwrap().abs() < 1e-15, "zero at the reference state");
    for t in &mut s.tf {
        *t += 10.0;
    }
    let rho = cpl.reactivity_of(&s).unwrap();
    println!("feedback: {rho:e} vs {:e}", -ALPHA * 10.0);
    assert!((rho + ALPHA * 10.0).abs() < 1e-15);
    assert!((cpl.node_powers().iter().sum::<f64>() / P0 - 1.0).abs() < 1e-14);
}

/// **Picard-converged coupling reproduces the monolithic solve; loose
/// coupling is first order.** +0.1 β at t = 0, 2 s. **Result
/// (2026-11-01):** Picard to 1e-12: worst power difference 3.8e-12, fuel
/// temperature 7.3e-15; P(2 s) = 3.4338 MW. Loose: 2.420e-4 at Δt 0.02 s,
/// 1.210e-4 at 0.01 s, ratio 2.001. Held.
///
/// **Re-measured 2026-11-10 (gh:#1319)**, after `teh-o-prke`'s U-235
/// default became ENDF/B-VIII.0 (beta 0.0065 -> 0.0065230, mean precursor
/// life 13.05 -> 11.06 s; both the coupled and the monolithic solve read
/// it): Picard worst power difference ~~3.8e-12~~ **4.2e-12**, fuel
/// temperature ~~7.3e-15~~ **5.9e-15**; P(2 s) ~~3.4338~~ **3.4585 MW**
/// (+0.72 %: a 0.35 % larger step and shorter-lived groups). Loose:
/// ~~2.420e-4~~ **2.574e-4** at 0.02 s, ~~1.210e-4~~ **1.286e-4** at
/// 0.01 s, ratio 2.001. Held. This movement was not predicted on the issue
/// beforehand (the test was found after the predictions were posted).
#[test]
fn coupled_matches_monolithic_and_loose_coupling_is_first_order() {
    let tight = PicardControl { max_iterations: 50, tolerance: 1e-12 };
    let (wp, wt, p_end) = compare(0.01, tight);
    println!("tight: worst power {wp:e}, worst fuel T {wt:e}; P(2 s) {:.4} MW", p_end / 1e6);
    assert!(wp < 1e-8 && wt < 1e-8);
    let loose = PicardControl { max_iterations: 1, tolerance: 0.0 };
    let (a, _, _) = compare(0.02, loose);
    let (b, _, _) = compare(0.01, loose);
    println!("loose: worst power difference {a:e} at dt 0.02 s, {b:e} at 0.01 s; ratio {:.3}", a / b);
    assert!((1.7..2.3).contains(&(a / b)));
    // An unmet tolerance is reported.
    let mut cpl = coupling();
    cpl.set_external_reactivity(Ratio::new::<ratio>(0.003));
    let mut side = LumpedLoop::steady(P0);
    let r = cpl.coupled_step(&mut side, Time::new::<second>(0.5), PicardControl { max_iterations: 2, tolerance: 1e-15 });
    assert!(matches!(r, Err(SystemCouplingError::NotConverged { .. })));
    assert!(node_mesh(&[]).is_err());
}
