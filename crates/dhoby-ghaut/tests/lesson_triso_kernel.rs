//! Reaches every function of the TRISO-kernel chapter's generator
//! (`examples/lesson_triso_kernel/kernel.rs`) and checks each against
//! something it can fail on: Booth's closed-form limits, the code's own
//! groups, a hand-solved conduction case, and tampines' particle solution.
//! The generated page data are checked by
//! `docs/site/science/fission-products/check-fission.js`.
//!
//! Results 2026-10-20: all 5 pass (`cargo test --release
//! --ignore-rust-version -p dhoby-ghaut --test lesson_triso_kernel`).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_triso_kernel/kernel.rs"]
mod kernel;

use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::Nuclide;
use kernel::*;

#[test]
fn booth_limits() {
    // Short time: f ~ 6 sqrt(x/pi) - 3x (Crank, as the derivation doc gives).
    let x = 1e-4;
    let short = 6.0 * (x / std::f64::consts::PI).sqrt() - 3.0 * x;
    assert!((booth_fraction(x) - short).abs() < 1e-6, "{}", booth_fraction(x));
    assert!(booth_fraction(10.0) > 0.999999);
    // R/B: 3/mu at large mu, 1 at small.
    assert!((booth_rb(1e4) * 1e4 / 3.0 - 1.0).abs() < 1e-3);
    assert!((booth_rb(1e-2) - 1.0).abs() < 1e-4);
}

#[test]
fn the_codes_lumping() {
    let t: Vec<f64> = (0..=8).map(|i| 400.0 + 200.0 * i as f64).collect();
    let c = d_classes(&t, 100);
    assert_eq!(c.len(), 5);
    assert!(c.iter().any(|k| k.members == vec![34, 36, 52, 53, 54]));
    assert!(c.iter().any(|k| k.members == vec![46, 47]));
    let (k, g) = d_curves(54, &t);
    assert_eq!(k, g, "Kr/Xe/I/Te/Se: graphite D is kernel D");
    assert_eq!(group_name(54), "NobleGas");
    assert_eq!(group_name(52), "Halogen");
    assert!(!plates_out(54) && plates_out(53) && plates_out(55));
    assert_eq!(SYMBOLS[54], "Xe");
}

#[test]
fn monte_carlo_against_booth() {
    let a = 2.5e-4;
    let (d, mc, booth) = mc_vs_booth(Nuclide::Cs137, a, 1200.0, 1.0, 1, 1);
    assert!(d > 0.0 && (0.0..=1.0).contains(&mc) && booth >= 0.0);
    let t = 0.04 * a * a / d;
    let (_, mc, booth) = mc_vs_booth(Nuclide::Cs137, a, 1200.0, t, 2000, 5);
    let se = (booth * (1.0 - booth) / 2000.0).sqrt();
    assert!((mc - booth).abs() < 4.0 * se, "MC {mc} Booth {booth}");
}

#[test]
fn kernel_conduction() {
    // Constant k: the analytic rise q a^2 / (6 k).
    let prof = sphere_profile(6e8, 2.5e-4, 1000.0, 400, |_| 3.0);
    let exact = 6e8 * 2.5e-4 * 2.5e-4 / (6.0 * 3.0);
    assert!((prof[0].1 - 1000.0 - exact).abs() < 1e-6 * exact.max(1.0), "{}", prof[0].1);
    assert_eq!(prof.last().unwrap(), &(2.5e-4, 1000.0));
    let (q, p) = htr10_kernel_power_density();
    assert!(q > 0.0 && p > 0.0);
    let k = k_uo2(1000.0);
    assert!(k > 1.0 && k < 10.0, "{k}");
    // Against tampines' own particle solution, to 0.5 K.
    let rise = sphere_profile(q, 2.5e-4, 1200.0, 200, k_uo2)[0].1 - 1200.0;
    let (c, s) = tampines_particle(p, 1200.0);
    assert!((rise - (c - s)).abs() < 0.5, "{rise} vs {}", c - s);
}

#[test]
fn half_lives() {
    let lib = DecayLibrary::new();
    assert!(half_life_s(&lib, Nuclide::Xe133).unwrap() > 0.0);
    assert_eq!(half_life_s(&lib, Nuclide::Xe136), None);
}
