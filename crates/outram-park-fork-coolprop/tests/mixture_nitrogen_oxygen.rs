//! V&V — the multi-fluid mixture engine (`mixtures`, bead op-kbc.16) against
//! the one real binary pair ported (Nitrogen–Oxygen, `F = 0`) and well-known
//! properties of air.
//!
//! ## Methodology
//!
//! Two independent checks, neither needing an external oracle:
//!
//! 1. **Pure-component limit**: at `x = [1, 0]`, the GERG-2008 reducing
//!    functions `T_r(x)`, `ρ_r(x)` must reduce *exactly* to pure Nitrogen's own
//!    `T_c`, `ρ_c` (every cross/mixing term in `reducing.rs` is multiplied by
//!    `x_j = 0`) — a code-structural invariant, not an empirical reference.
//! 2. **Air-like composition vs literature**: a 79%/21% N₂/O₂ mixture (a
//!    standard simplified-air proxy) at `T = 300 K`, `ρ_molar = 40 mol/m³`
//!    (≈ 1 atm) is checked against two independent, very well-known air
//!    properties: the ideal-gas-ish pressure (should land near 1 atm) and the
//!    speed of sound in air at 300 K (widely cited ≈ 347 m/s).
//!
//! ## Results (2026-07-10, this port)
//!
//! Pure limit: `T_r([1,0])` and `ρ_r([1,0])` match pure N₂'s `T_c`/`ρ_c` to
//! `<1e-12` relative error. Air-like state: `p` = 99 746 Pa (0.984 atm, correct
//! ballpark for 40 mol/m³ at 300 K including the real-gas/departure-free N₂-O₂
//! surface); `w` = 347.19 m/s vs the literature value ≈ 347 m/s (<0.1%).

use outram_park_fork_coolprop::mixtures::Mixture;
use outram_park_fork_coolprop::Fluid;

#[test]
fn pure_nitrogen_limit_is_exact() {
    let mix = Mixture::new(vec![Fluid::Nitrogen, Fluid::Oxygen], vec![1.0, 0.0]).unwrap();
    let n2 = Fluid::Nitrogen.eos();

    let tr = mix.reducing_temperature();
    let rho_r = mix.reducing_density();
    println!("T_r([1,0]) = {tr} (N2 Tc = {}), rho_r([1,0]) = {rho_r} (N2 rho_c = {})", n2.t_critical, n2.rho_critical);

    assert!((tr - n2.t_critical).abs() / n2.t_critical < 1e-12);
    assert!((rho_r - n2.rho_critical).abs() / n2.rho_critical < 1e-12);
}

#[test]
fn air_like_mixture_matches_known_air_properties() {
    let air = Mixture::new(vec![Fluid::Nitrogen, Fluid::Oxygen], vec![0.79, 0.21]).unwrap();
    let state = air.state_trho_molar(300.0, 40.0).expect("valid air-like state");

    println!("air-like (300K, 40 mol/m3): p={:.3} Pa, cp={:.4} J/(mol.K), w={:.4} m/s", state.pressure, state.cp, state.speed_of_sound);

    // ~1 atm at these (T, rho_molar) for a near-ideal diatomic mixture.
    assert!((state.pressure - 101_325.0).abs() / 101_325.0 < 0.02, "p = {} Pa, expected near 1 atm", state.pressure);

    // Speed of sound in air at 300 K is widely cited as ~347 m/s.
    assert!((state.speed_of_sound - 347.0).abs() / 347.0 < 0.01, "w = {} m/s, expected ~347 m/s", state.speed_of_sound);

    assert!(state.cp > state.cv, "cp {} should exceed cv {}", state.cp, state.cv);
}

#[test]
fn malformed_composition_is_rejected() {
    assert!(Mixture::new(vec![Fluid::Nitrogen], vec![1.0]).is_err(), "single-component 'mixture' should be rejected");
    assert!(Mixture::new(vec![Fluid::Nitrogen, Fluid::Oxygen], vec![1.0]).is_err(), "mismatched array lengths should be rejected");
}
