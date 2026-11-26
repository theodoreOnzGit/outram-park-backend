// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Tests of `boiling_trace` and `boiling_trace_chf` (gh:#1222). Unit-level: branch selection, the
//! upstream literals (film-condensation blend, the `-1e-3` floor), Gorenflo `TCHF` against the
//! pool-boiling closure, and the CHF-bool hand-over to the vapour side. The comparison with the real
//! binary is `tests/genfoam_psbt_trace_vs_upstream.rs`.

use super::boiling_trace::{multi_regime_boiling_trace, TraceBoilingInputs};
use super::boiling_trace_chf::{
    gorenflo_t_chf, multi_regime_boiling_trace_chf, multi_regime_boiling_vapour_trace, TraceChfInputs,
};
use super::fs_htc::PoolBoilingHtc;
use uom::si::f64::{Length, Pressure, TemperatureInterval};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::temperature_interval::kelvin as dk;

fn gorenflo(p: f64, t_wall: f64, t_sat: f64) -> f64 {
    PoolBoilingHtc::Gorenflo { surface_roughness: Length::new::<meter>(4e-7) }
        .heat_transfer_coefficient_from_delta_t(TemperatureInterval::new::<dk>(t_wall - t_sat), Pressure::new::<pascal>(p))
        .get::<uom::si::heat_transfer::watt_per_square_meter_kelvin>()
}

fn base(t_wall: f64) -> TraceBoilingInputs {
    TraceBoilingInputs {
        alpha_other: 0.1,
        t_wall,
        t_fluid: 600.0,
        t_sat: 614.0,
        htc_2p_fc: 30_000.0,
        htc_film_condensation: None,
        has_onb_model: true,
    }
}

#[test]
fn trace_below_saturation_and_below_onset_return_the_forced_convection_coefficient() {
    let never = |_: f64| -> f64 { panic!("the ONB model is not evaluated below saturation") };
    let r = multi_regime_boiling_trace(&base(610.0), never, |_| 0.0, None::<fn(f64) -> f64>);
    assert_eq!(r.htc, 30_000.0);
    assert_eq!(r.q_subcooled_dmdt, None);
    // above saturation, below the onset: the pool-boiling model is not evaluated
    let r = multi_regime_boiling_trace(&base(616.0), |_| 620.0, |_| panic!("not evaluated"), None::<fn(f64) -> f64>);
    assert_eq!(r.htc, 30_000.0);
}

#[test]
fn trace_nucleate_boiling_is_the_cubic_superposition_and_writes_the_subcooled_term() {
    let p = 14.7e6;
    let t_w = 619.0;
    let t_onb = 616.5;
    let inp = base(t_w);
    let pool = |tw: f64| gorenflo(p, tw, inp.t_sat);
    let r = multi_regime_boiling_trace(&inp, |_| t_onb, pool, Some(|_q: f64| 0.25));
    let q_fc: f64 = 30_000.0 * (t_w - 600.0);
    let q_pb = pool(t_w) * (t_w - 614.0);
    let q_bi = pool(t_onb) * (t_onb - 614.0);
    let q_nb = (q_fc.powf(3.0) + (q_pb - q_bi).powf(3.0)).powf(1.0 / 3.0);
    assert!((r.htc - q_nb / (t_w - 600.0)).abs() <= 1e-12 * r.htc);
    let q_sc = r.q_subcooled_dmdt.expect("boiling branch writes dmdtW");
    assert!((q_sc - 0.25 * (q_nb - q_fc)).abs() <= 1e-9 * q_sc.abs());
    // without a sub-cooled fraction model f = 1
    let r1 = multi_regime_boiling_trace(&inp, |_| t_onb, pool, None::<fn(f64) -> f64>);
    assert!((r1.q_subcooled_dmdt.expect("written") - (q_nb - q_fc)).abs() <= 1e-9 * q_nb);
}

/// Upstream's literal `dT = (dT >= 0) ? max(dT, 1e-3) : min(-dT, -1e-3)`: for `T_w < T_f` the divisor is
/// exactly `-1e-3` (here the wall is above saturation but the fluid hotter than the wall).
#[test]
fn trace_negative_wall_to_fluid_difference_uses_the_literal_minus_one_milli() {
    let mut inp = base(619.0);
    inp.t_fluid = 625.0;
    inp.t_sat = 614.0;
    let pool = |tw: f64| gorenflo(14.7e6, tw, 614.0);
    let r = multi_regime_boiling_trace(&inp, |_| 616.5, pool, None::<fn(f64) -> f64>);
    let q_fc: f64 = 30_000.0 * (619.0 - 625.0);
    let q_pb = pool(619.0) * 5.0;
    let q_bi = pool(616.5) * 2.5;
    let q_nb = (q_fc.powf(3.0) + (q_pb - q_bi).powf(3.0)).powf(1.0 / 3.0);
    assert!((r.htc - q_nb / -1e-3).abs() <= 1e-9 * r.htc.abs());
}

#[test]
fn trace_film_condensation_uses_the_literal_blend() {
    let mut inp = base(610.0);
    inp.htc_film_condensation = Some(5_000.0);
    inp.alpha_other = 0.85;
    let r = multi_regime_boiling_trace(&inp, |_| 0.0, |_| 0.0, None::<fn(f64) -> f64>);
    let f = 0.9 - 0.85 / 0.1;
    assert_eq!(r.htc, f * 30_000.0 + (1.0 - f) * 5_000.0);
    inp.alpha_other = 0.95;
    assert_eq!(multi_regime_boiling_trace(&inp, |_| 0.0, |_| 0.0, None::<fn(f64) -> f64>).htc, 5_000.0);
    inp.alpha_other = 0.5;
    assert_eq!(multi_regime_boiling_trace(&inp, |_| 0.0, |_| 0.0, None::<fn(f64) -> f64>).htc, 30_000.0);
}

#[test]
fn gorenflo_t_chf_is_the_wall_temperature_that_delivers_the_critical_heat_flux() {
    let (p, t_sat, q_chf) = (14.7e6, 614.0, 2.0e6);
    let t_chf = gorenflo_t_chf(t_sat, p, q_chf, 4e-7);
    let q = gorenflo(p, t_chf, t_sat) * (t_chf - t_sat);
    assert!((q - q_chf).abs() <= 1e-9 * q_chf, "q = {q}");
    // a rougher surface boils at a lower superheat
    assert!(gorenflo_t_chf(t_sat, p, q_chf, 4e-6) < t_chf);
}

fn chf_inputs(t_wall: f64, q_chf: Option<f64>) -> TraceChfInputs {
    TraceChfInputs {
        alpha_other: 0.1,
        t_wall,
        t_fluid: 600.0,
        t_sat: 614.0,
        p: 14.7e6,
        area: 150.0,
        latent_heat: 1.0e6,
        htc_2p_fc: 30_000.0,
        htc_film_condensation: None,
        has_onb_model: true,
        q_chf,
        roughness: 4e-7,
    }
}

#[test]
fn trace_chf_pre_chf_equals_the_trace_model_with_the_documented_dmdtw() {
    let p = 14.7e6;
    let pool = |tw: f64| gorenflo(p, tw, 614.0);
    let inp = chf_inputs(619.0, None);
    let r = multi_regime_boiling_trace_chf(
        &inp,
        |_| 616.5,
        pool,
        Some(|_q: f64| 0.5),
        None::<fn() -> f64>,
        |_| (0.0, 0.0),
    );
    let t = multi_regime_boiling_trace(
        &TraceBoilingInputs {
            alpha_other: 0.1,
            t_wall: 619.0,
            t_fluid: 600.0,
            t_sat: 614.0,
            htc_2p_fc: 30_000.0,
            htc_film_condensation: None,
            has_onb_model: true,
        },
        |_| 616.5,
        pool,
        Some(|_q: f64| 0.5),
    );
    assert_eq!(r.htc, t.htc);
    assert_eq!(r.chf_bool, 0.0);
    assert_eq!(r.dmdt_w, Some(150.0 * t.q_subcooled_dmdt.expect("written") / 1.0e6));
}

#[test]
fn trace_chf_post_chf_branches_hand_over_the_chf_bool_to_the_vapour_side() {
    let p = 14.7e6;
    let pool = |tw: f64| gorenflo(p, tw, 614.0);
    let q_chf = 1.0e6;
    let t_chf = gorenflo_t_chf(614.0, p, q_chf, 4e-7);
    let t_lf = t_chf + 40.0;
    let annular = |tw: f64| (2_000.0 + tw, 0.5 * tw);
    // above TCHF and above the Leidenfrost temperature: film boiling
    let hot = chf_inputs(t_lf + 10.0, Some(q_chf));
    let r = multi_regime_boiling_trace_chf(&hot, |_| 616.0, pool, None::<fn(f64) -> f64>, Some(|| t_lf), annular);
    assert_eq!(r.chf_bool, 1.0);
    assert_eq!(r.htc, 2_000.0 + hot.t_wall);
    assert_eq!(r.dmdt_w, Some(150.0 * (0.5 * hot.t_wall) * (hot.t_wall - 600.0) / 1.0e6));
    let hv = multi_regime_boiling_vapour_trace(hot.t_wall, r.chf_bool, r.wf_tb, r.t_leidenfrost, |tw| 2_000.0 + tw);
    assert_eq!(hv, r.htc);
    // between TCHF and T_LF: transition boiling with the quadratic weight
    let mid = chf_inputs(t_chf + 10.0, Some(q_chf));
    let r = multi_regime_boiling_trace_chf(&mid, |_| 616.0, pool, None::<fn(f64) -> f64>, Some(|| t_lf), annular);
    // Twi < TLFi is the transition branch (T_w = T_CHF + 10 < T_LF = T_CHF + 40)
    assert_eq!(r.chf_bool, 2.0);
    let wf = ((mid.t_wall - t_lf) / (t_chf - t_lf)).powi(2);
    assert!((r.wf_tb - wf).abs() <= 1e-12);
    let hv = multi_regime_boiling_vapour_trace(mid.t_wall, r.chf_bool, r.wf_tb, r.t_leidenfrost, |tw| 2_000.0 + tw);
    assert_eq!(hv, (1.0 - r.wf_tb) * (2_000.0 + t_lf));
    // pre-CHF: nothing to the vapour
    assert_eq!(multi_regime_boiling_vapour_trace(619.0, 0.0, 0.0, 0.0, |_| 1.0), 0.0);
}

#[test]
fn trace_chf_without_a_leidenfrost_model_has_no_hysteresis_zone() {
    let p = 14.7e6;
    let pool = |tw: f64| gorenflo(p, tw, 614.0);
    let q_chf = 1.0e6;
    let t_chf = gorenflo_t_chf(614.0, p, q_chf, 4e-7);
    let inp = chf_inputs(t_chf + 5.0, Some(q_chf));
    let r = multi_regime_boiling_trace_chf(&inp, |_| 616.0, pool, None::<fn(f64) -> f64>, None::<fn() -> f64>, |_| (7.0, 0.0));
    // T_LF = T_w: `Twi < TLFi` is false, straight to film boiling
    assert_eq!(r.chf_bool, 1.0);
    assert_eq!(r.htc, 7.0);
    assert_eq!(r.t_leidenfrost, inp.t_wall);
}
