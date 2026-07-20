//! V&V — Helium pressure from `(T, ρ)` against CoolProp's tabulated states.
//!
//! ## Methodology
//!
//! Same as `water_reference.rs`: pressure is reference-state-independent, so it
//! is the cleanest code-to-code check. Reference `(T, ρ, p)` triples are
//! CoolProp's tabulated Helium states (`dev/fluids/Helium.json`, extracted at
//! authoring time — not a runtime input).
//!
//! Helium's EOS uses **only** Power + Gaussian residual terms (no non-analytic
//! critical-region term), so — unlike Water — it is fully accurate with the
//! current engine **including at the critical point**, which is tested here.
//!
//! ## Results (2026-07-08, this port)
//!
//! Triple-liquid, triple-vapour and the **critical point** all reproduce
//! CoolProp's tabulated pressures; see the asserted tolerances.

// Reference values are transcribed verbatim from CoolProp's JSON.
#![allow(clippy::inconsistent_digit_grouping)]

use outram_park_fork_coolprop::{state_trho, Fluid};

fn check(label: &str, t: f64, rho: f64, p_ref: f64, tol: f64) {
    let s = state_trho(Fluid::Helium, t, rho);
    let rel = (s.pressure - p_ref).abs() / p_ref;
    println!("{label:14} p = {:.6} Pa  (ref {:.6})  rel = {:.3e}", s.pressure, p_ref, rel);
    assert!(rel < tol, "{label}: pressure {} Pa vs {} Pa (rel {:.3e} > {:.1e})", s.pressure, p_ref, rel, tol);
}

#[test]
fn helium_triple_vapor_pressure() {
    check("triple_vapor", 2.1768, 1.174_398_098, 5039.330_380_576_782, 1e-3);
}

#[test]
fn helium_triple_liquid_pressure() {
    // Saturated liquid He at the triple line — large cancellation in p; still
    // reproduces the reference to well within 1e-3.
    check("triple_liquid", 2.1768, 146.016_302_977, 5039.330_380_576_782, 1e-3);
}

#[test]
fn helium_critical_pressure() {
    // Helium has no non-analytic term, so the critical point is exact here.
    check("critical", 5.1953, 72.567_174_260, 228_326.069_909_028_07, 1e-3);
}

#[test]
fn helium_state_is_physical() {
    // Supercritical helium at 300 K, 1.6 kg/m^3.
    let s = state_trho(Fluid::Helium, 300.0, 1.6);
    assert!(s.pressure > 0.0 && s.pressure.is_finite());
    assert!(s.cp > 0.0 && s.cv > 0.0 && s.cp >= s.cv, "cp={} cv={}", s.cp, s.cv);
    assert!(s.speed_of_sound > 0.0 && s.speed_of_sound.is_finite(), "w={}", s.speed_of_sound);
}
