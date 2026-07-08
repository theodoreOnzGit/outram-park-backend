//! V&V — Water (IAPWS-95) pressure from `(T, ρ)` against CoolProp's own
//! tabulated reference states.
//!
//! ## Methodology
//!
//! Pressure is reference-state-independent (unlike absolute h/s/u), so it is
//! the cleanest code-to-code check of the residual Helmholtz EOS. The
//! reference `(T, ρ, p)` triples are CoolProp's tabulated states for Water
//! (`dev/fluids/Water.json`, extracted at authoring time — the JSON is not a
//! runtime input). Both points sit at the triple-point temperature, far from
//! the critical point, so the not-yet-implemented non-analytic terms are
//! negligible there (ψ = exp(−C(δ−1)²−D(τ−1)²) ≈ 0).
//!
//! ## Results (2026-07-08, this port)
//!
//! Printed by `cargo test -p outram-park-fork-coolprop --test water_reference
//! -- --nocapture`. Both points reproduce CoolProp's tabulated saturation
//! pressure 611.6548 Pa; see the asserted tolerances below. The critical /
//! near-critical points are intentionally NOT tested yet (they need the
//! non-analytic terms — tracked, bead op-kbc).

use outram_park_fork_coolprop::{state_trho, Fluid};

const P_TRIPLE: f64 = 611.654_800_896_868_4; // Pa (CoolProp Water triple line)

#[test]
fn water_triple_vapor_pressure() {
    // Low-density saturated vapour at the triple point: T = 273.16 K,
    // rho = 0.004855 kg/m^3. Little cancellation in p = rho*R*T*(1 + d*ar_d).
    let s = state_trho(Fluid::Water, 273.16, 0.004_855_058_1);
    let rel = (s.pressure - P_TRIPLE).abs() / P_TRIPLE;
    println!("triple_vapor  p = {:.6} Pa  (ref {:.6})  rel = {:.3e}", s.pressure, P_TRIPLE, rel);
    assert!(rel < 1e-3, "triple-vapour pressure {} Pa vs {} Pa (rel {:.3e})", s.pressure, P_TRIPLE, rel);
}

#[test]
fn water_triple_liquid_pressure() {
    // Saturated liquid at the triple point: T = 273.16 K, rho = 999.7925 kg/m^3.
    // p here is a ~2e5-fold near-cancellation of rho*R*T, so this is a very
    // stringent test of the residual delta-derivative.
    let s = state_trho(Fluid::Water, 273.16, 999.792_520);
    let rel = (s.pressure - P_TRIPLE).abs() / P_TRIPLE;
    println!("triple_liquid p = {:.6} Pa  (ref {:.6})  rel = {:.3e}", s.pressure, P_TRIPLE, rel);
    // Achieves ~1e-4 in practice (the ~2e5x cancellation is not a problem —
    // the residual delta-derivative is essentially exact); gate at 1e-3.
    assert!(rel < 1e-3, "triple-liquid pressure {} Pa vs {} Pa (rel {:.3e})", s.pressure, P_TRIPLE, rel);
}

#[test]
fn water_state_is_physical() {
    // Ambient-ish liquid water: finite, positive, sane properties.
    let s = state_trho(Fluid::Water, 300.0, 996.5);
    assert!(s.pressure.is_finite());
    assert!(s.cp > 0.0 && s.cv > 0.0, "cp={} cv={}", s.cp, s.cv);
    assert!(s.cp >= s.cv, "cp {} < cv {}", s.cp, s.cv);
    assert!(s.speed_of_sound.is_finite() && s.speed_of_sound > 0.0, "w={}", s.speed_of_sound);
}
