//! Reaches every function of the breathing lesson's generator
//! (`examples/lesson_breathing/breath.rs`) and checks each against
//! something it can fail on: the well-mixed gas against the written-out
//! formula and against the closed form for a release that is all there at
//! the start (where the product is exact), and the ordering the lesson rests
//! on. The generated page data are checked by
//! `docs/site/science/fission-products/check-breathing.js`.
//!
//! Results 2026-11-04: all pass (`cargo test --release -p dhoby-ghaut --test
//! lesson_breathing`).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_breathing/breath.rs"]
mod breath;

#[test]
fn release_present_from_the_start_vents_exactly_one_minus_t0_over_t() {
    // Everything is in the gas at t = 0 (a step in R at the first interval
    // is not that, so put R = 1 throughout): then the well-mixed gas and the
    // product agree, 1 − T0/Tmax.
    let hours = [0.0, 5.0, 10.0, 15.0];
    let t_c = [900.0, 1000.0, 1100.0, 1100.0];
    let release = [1.0, 1.0, 1.0, 1.0];
    let m = breath::well_mixed(&hours, &t_c, &release, 0.0, 400);
    for (j, t) in t_c.iter().enumerate() {
        let tmax = t_c[..=j].iter().cloned().fold(f64::MIN, f64::max) + 273.15;
        assert!((m[j] - (1.0 - 1173.15 / tmax)).abs() < 1e-9, "{j}: {} vs {}", m[j], 1.0 - 1173.15 / tmax);
        let _ = t;
    }
}

#[test]
fn well_mixed_matches_the_formula_and_stays_below_the_product() {
    let b = breath::breath("Cs-137", 1600.0, 200);
    let f = breath::mixed_formula(&b.hours, &b.t_c, &b.release, 200);
    for (x, y) in b.mixed_no_decay.iter().zip(&f) {
        assert!((x - y).abs() < 1e-6, "{x} vs {y}");
    }
    // Against the ideal-gas fraction the product is an upper bound everywhere.
    for i in 0..b.hours.len() {
        if b.vent[i].is_some() {
            assert!(b.vent_exact[i] * b.release[i] + 1e-15 >= b.mixed_no_decay[i], "{} h", b.hours[i]);
        }
    }
    // Decay in the gas only lowers what is carried out.
    assert!(b.mixed.iter().zip(&b.mixed_no_decay).all(|(a, n)| *a <= *n + 1e-15));
    assert_eq!(breath::NUCLIDES.len(), 4);
    assert!(b.atops.iter().filter(|a| a.is_some()).count() > 0);
}
