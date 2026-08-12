//! Smoke test — every wired fluid evaluates to a physically sane state.
//!
//! ## Methodology
//!
//! All of CoolProp's pure fluids are wired into [`Fluid`] (see
//! `dev/regen_all.py`). This test iterates `Fluid::ALL` and, for each fluid,
//! evaluates `state_trho` at that fluid's own critical point
//! (`t_critical`, `rho_critical` from its hardcoded [`FluidEos`]) and checks:
//!
//! - the computed pressure is finite and positive, and
//! - it reproduces the tabulated critical pressure `p_critical` to within 25 %.
//!
//! The 25 % band is deliberately loose: at the *exact* critical point the EOS
//! is at an inflection (`∂p/∂ρ → 0`, so `c_p` and the speed of sound diverge —
//! not checked here), and the EOS critical pressure differs from the
//! *experimental* tabulated `p_critical` by up to a few percent for some fluids
//! (and more for the handful of pseudo-pure refrigerant blends). The point of
//! this test is not accuracy — the per-fluid reference tests
//! (`water_reference`, `helium_reference`, `term_types_reference`) cover that —
//! but to catch a fluid whose data fails to evaluate at all (NaN/inf/≤0), which
//! would flag a term-type mishandled for that specific fluid.
//!
//! ## Results (2026-07-08)
//!
//! All 137 fluids evaluate to a finite, positive critical pressure within the
//! 25 % band.

use outram_park_fork_coolprop::{state_trho, Fluid};

#[test]
fn every_fluid_evaluates_at_its_critical_point() {
    let mut checked = 0;
    for &f in Fluid::ALL {
        let eos = f.eos();
        let rho = eos.rho_critical * eos.molar_mass; // mol/m³ → kg/m³
        let s = state_trho(f, eos.t_critical, rho);
        assert!(
            s.pressure.is_finite() && s.pressure > 0.0,
            "{}: non-physical critical pressure {} Pa",
            f.name(),
            s.pressure
        );
        let rel = (s.pressure - eos.p_critical).abs() / eos.p_critical;
        assert!(
            rel < 0.25,
            "{}: critical pressure {:.3e} Pa vs tabulated {:.3e} Pa (rel {:.2e})",
            f.name(),
            s.pressure,
            eos.p_critical,
            rel
        );
        checked += 1;
    }
    assert_eq!(checked, Fluid::ALL.len());
    assert!(
        checked >= 137,
        "expected all fluids wired, only {checked} checked"
    );
}
