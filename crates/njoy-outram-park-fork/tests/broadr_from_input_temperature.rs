//! BROADR broadens from the input's own temperature, `tempef = T − tempin`
//! (`broadr.f90:605-610`). GitHub #617.
//!
//! # Methodology
//!
//! A synthetic material (AWR 236.0058, a constant 10 b plus one Lorentzian
//! resonance of peak 2e4 b, Γ = 0.03 eV at 6.67 eV, on a fine grid) is
//! broadened two ways: 0 K → 900 K directly, and 0 K → 293.6 K → 900 K.
//! Free-gas broadening is a Gaussian convolution in velocity whose variances
//! add, so the two must agree to the thinning tolerance (`errthn = 0.001`
//! per pass). The check is discriminating: before the fix the second step ran
//! the full 900 K kernel on 293.6 K data, an effective 1193.6 K, whose peak
//! is about 13 % lower (the Doppler width goes as √T and Δ ≫ Γ here).
//!
//! # Results (2026-11-10)
//!
//! Predicted before running: the two routes agree to under 0.5 % at the
//! peak; a direct 1193.6 K peak sits about 13 % below. Measured: peak
//! 4792.074 b (0 → 900 K) against 4791.523 b (0 → 293.6 → 900 K), −1.15e-4;
//! 0 → 1193.6 K gives 4253.361 b, −11.2 %, which is what the second call
//! returned before the fix. Every caller in the workspace broadens 0 K data
//! once, where `tempef = T` exactly, so no recorded number moves.

use petir::mathf::RealMath;
use njoy_outram_park_fork::{
    broadr::broaden_result,
    reconr::{mf1::MaterialInfo, ReconrResult, ReconrSection},
    MtReaction,
};

const E0: f64 = 6.67;
const GAMMA: f64 = 0.03;

fn sigma(e: f64) -> f64 {
    let h = GAMMA / 2.0;
    10.0 + 2.0e4 * h * h / ((e - E0) * (e - E0) + h * h)
}

fn material_0k() -> ReconrResult {
    let mut grid: Vec<f64> = (0..=12 * 200).map(|i| 1.0e-5 * 10f64.r_powf(f64::from(i) / 200.0)).collect();
    grid.extend((0..=4000).map(|i| E0 - 2.0 + 4.0 * f64::from(i) / 4000.0));
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup_by(|a, b| (*a - *b).abs() < 1e-12 * b.abs());
    let pairs: Vec<(f64, f64)> = grid.iter().map(|&e| (e, sigma(e))).collect();
    let sec = |mt: i32| ReconrSection { lr: 0, mt: MtReaction::from_any(mt), qi: 0.0, pairs: pairs.clone() };
    ReconrResult {
        material: MaterialInfo { za: 92238.0, awr: 236.0058, lrp: 0, lfi: 0, nlib: 0, elis: 0.0, nfor: 6, emax: 2.0e7 },
        sections: vec![sec(1), sec(2)],
        resonance_upper_limit: None,
        unresolved_table: None,
        temperature_k: 0.0,
    }
}

fn at(r: &ReconrResult, e: f64) -> f64 {
    let s = &r.sections.iter().find(|s| s.mt.number() == 2).unwrap().pairs;
    let i = s.partition_point(|p| p.0 < e);
    let ((x0, y0), (x1, y1)) = (s[i - 1], s[i]);
    y0 + (e - x0) * (y1 - y0) / (x1 - x0)
}

#[test]
fn broadening_twice_matches_broadening_once() {
    let r0 = material_0k();
    let once = broaden_result(&r0, 900.0);
    let warm = broaden_result(&r0, 293.6);
    let twice = broaden_result(&warm, 900.0);
    let hot = broaden_result(&r0, 1193.6);
    assert_eq!((warm.temperature_k, twice.temperature_k, once.temperature_k), (293.6, 900.0, 900.0));

    let (p1, p2, p_hot, p_warm) = (at(&once, E0), at(&twice, E0), at(&hot, E0), at(&warm, E0));
    let rel = (p2 - p1) / p1;
    println!(
        "peak at {E0} eV: 0->900 K {p1:.4} b, 0->293.6->900 K {p2:.4} b ({rel:+.2e}), \
         0->1193.6 K {p_hot:.4} b ({:+.3}), 293.6 K {p_warm:.4} b",
        (p_hot - p1) / p1
    );
    assert!(rel.abs() < 5.0e-3, "two-step peak {p2} vs one-step {p1}");
    assert!((p_hot - p1) / p1 < -0.05, "the check can tell 900 K from 1193.6 K");
    for e in [E0 - 0.3, E0 + 0.3, 1.0, 100.0] {
        let (a, b) = (at(&once, e), at(&twice, e));
        assert!(((b - a) / a).abs() < 5.0e-3, "E = {e}: {b} vs {a}");
    }
}

#[test]
fn not_above_the_input_temperature_passes_the_data_through() {
    let warm = broaden_result(&material_0k(), 600.0);
    for t in [600.0, 300.0, 0.0] {
        let r = broaden_result(&warm, t);
        assert_eq!(r.temperature_k, 600.0, "still the 600 K data, asked for {t} K");
        assert_eq!(r.sections[1].pairs, warm.sections[1].pairs);
    }
}
