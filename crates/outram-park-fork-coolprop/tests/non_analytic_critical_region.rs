//! V&V — the non-analytic critical-region residual term
//! ([`eos::ResidualTerm::NonAnalytic`], implemented via
//! `eos::accumulate_non_analytic`, bead op-kbc.6) against Water's own defining
//! property: `p(T_c, ρ_c) = p_c` exactly, by IAPWS-95's construction.
//!
//! ## Methodology
//!
//! IAPWS-95 is *fit* so that the critical point is exactly reproduced: at
//! `T = T_c = 647.096 K`, `ρ = ρ_c = 322.0 kg/m³` (`= ρ_{c,molar} · M`), the
//! EOS must give `p = p_c = 22 064 000 Pa` to the limits of double precision.
//! Before this term was implemented, `ResidualTerm::NonAnalytic::accumulate`
//! was a no-op (see `src/eos.rs` git history), and this exact point was known
//! to carry a ~1e-4 relative residual (documented in
//! `tests/water_reference.rs` and the crate `CLAUDE.md`) — entirely
//! attributable to the missing critical-region term, since every other
//! residual/ideal term form is otherwise verified exact (see
//! `tests/term_types_reference.rs`, `tests/helium_reference.rs` — Helium has
//! no `NonAnalytic` term and was already machine-precision at its Tc).
//! `T_c, ρ_c` is exactly the branch point (`δ=1, τ=1`) `accumulate_non_analytic`
//! must offset away from by construction (see its doc comment) — so this is
//! also a direct test of that offset guard.
//!
//! A second point one degree below `T_c` on the critical isochore is checked
//! against a looser bound, since it is not a defining-fit point (no exact
//! reference value), only a monotonicity/sanity check that pressure decreases
//! smoothly moving down the isochore.
//!
//! ## Results (2026-07-10, this port)
//!
//! `p(T_c, ρ_c)` = 22 064 000.00000115 Pa vs the defining `p_c` =
//! 22 064 000 Pa — relative error `5.2e-14` (machine precision, printed by
//! `cargo test -p outram-park-fork-coolprop --test non_analytic_critical_region
//! -- --nocapture`). `p(T_c - 1 K, ρ_c)` = 21 796 687.6 Pa, below `p_c` as
//! physically expected (pressure drops moving below `T_c` along the critical
//! isochore, away from the coexistence-curve cusp).

use outram_park_fork_coolprop::{state_trho, Fluid};

#[test]
fn water_pressure_at_exact_critical_point() {
    let eos = Fluid::Water.eos();
    let rho_mass_c = eos.rho_critical * eos.molar_mass; // mol/m^3 -> kg/m^3
    assert!(
        (rho_mass_c - 322.0).abs() < 0.01,
        "sanity: water critical mass density should be ~322 kg/m^3, got {rho_mass_c}"
    );

    let s = state_trho(Fluid::Water, eos.t_critical, rho_mass_c);
    let rel = (s.pressure - eos.p_critical).abs() / eos.p_critical;
    println!(
        "p(Tc, rho_c) = {:.5} Pa (p_c = {} Pa), rel err = {:.3e}",
        s.pressure, eos.p_critical, rel
    );
    assert!(rel < 1e-9, "critical-point pressure should reproduce p_c to near machine precision, got rel err {rel:.3e}");
}

#[test]
fn water_pressure_just_below_critical_isochore_is_sane() {
    let eos = Fluid::Water.eos();
    let rho_mass_c = eos.rho_critical * eos.molar_mass;

    let s_at_tc = state_trho(Fluid::Water, eos.t_critical, rho_mass_c);
    let s_below = state_trho(Fluid::Water, eos.t_critical - 1.0, rho_mass_c);
    println!(
        "p(Tc, rho_c) = {:.3} Pa; p(Tc-1K, rho_c) = {:.3} Pa",
        s_at_tc.pressure, s_below.pressure
    );

    assert!(s_below.pressure.is_finite() && s_below.pressure > 0.0);
    assert!(
        s_below.pressure < s_at_tc.pressure,
        "pressure should drop moving below Tc on the critical isochore"
    );
    // 1K below Tc should still be within a fraction of a percent of p_c.
    let rel = (s_below.pressure - eos.p_critical).abs() / eos.p_critical;
    assert!(
        rel < 0.02,
        "p(Tc-1K, rho_c) = {} Pa is too far from p_c for a 1K step",
        s_below.pressure
    );
}
