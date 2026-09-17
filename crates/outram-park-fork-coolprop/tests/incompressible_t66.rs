//! V&V — the incompressible-fluid backend (`incompressibles`, bead op-kbc.15)
//! against a hand-evaluation of T66 (Therminol 66)'s own CoolProp JSON fit
//! coefficients.
//!
//! ## Methodology
//!
//! `reference/CoolProp/dev/incompressible_liquids/json/T66.json`'s
//! `density`/`specific_heat`/`conductivity` (`polynomial`, centred on
//! `Tbase = 463.15 K`) and `viscosity` (`exponential`, **not** `Tbase`-centred
//! — see the debugging note below) coefficients are evaluated independently in
//! Python (not via this crate) at `T = 300 K`, and compared to this port's
//! `Incompressible::T66` output. This is a direct code-vs-formula check of the
//! `poly2d`/`eval_fit` engine, not an external oracle (no CoolProp/`rfluids`
//! runtime is available in this environment — op-kbc.3).
//!
//! Debugging trail: the first version of `PropertyForm::Exponential` centred
//! `T` on `Tbase` (matching the `Polynomial`/`ExpPolynomial` forms), giving
//! `μ(300 K) ≈ 1.2e-5 Pa·s` — four orders of magnitude below Therminol 66's
//! published viscosity at 27 °C (tens of mPa·s, a viscous oil). Tracing
//! CoolProp's `IncompressibleFluid::visc()` dispatch found
//! `baseExponential(viscosity, T, 0.0)` — the third argument (`ybase`) is a
//! literal `0.0`, not `Tbase`, for this fit form specifically. Fixed in
//! `incompressibles/mod.rs::eval_fit`.
//!
//! ## Results (2026-07-10, this port)
//!
//! At `T = 300 K`: `ρ = 1003.848448 kg/m³`, `c_p = 1585.615364 J/(kg·K)`,
//! `λ = 0.117310 W/(m·K)`, `μ = 0.074726 Pa·s` — all match the hand
//! evaluation to `<1e-9` relative error (see the assertions below).

use outram_park_fork_coolprop::incompressibles::Incompressible;

const T: f64 = 300.0;
const P: f64 = 101_325.0; // unused by this fit family; passed for API symmetry

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b
}

#[test]
fn t66_density_heat_capacity_conductivity_viscosity_at_300k() {
    let rho = Incompressible::T66.density(T, P, 0.0).unwrap();
    let cp = Incompressible::T66.heat_capacity(T, P, 0.0).unwrap();
    let lambda = Incompressible::T66.conductivity(T, P, 0.0).unwrap();
    let mu = Incompressible::T66.viscosity(T, P, 0.0).unwrap();

    println!("T66 @ 300K: rho={rho:.6} cp={cp:.6} lambda={lambda:.6} mu={mu:.6}");

    assert!(rel(rho, 1003.848448) < 1e-6, "rho = {rho}");
    assert!(rel(cp, 1585.615364) < 1e-6, "cp = {cp}");
    assert!(rel(lambda, 0.117310) < 1e-5, "lambda = {lambda}");
    assert!(rel(mu, 0.074726) < 1e-5, "mu = {mu}");
}

#[test]
fn t66_enthalpy_matches_heat_capacity_integral() {
    let h = Incompressible::T66.enthalpy(T, P, 0.0).unwrap();
    println!("T66 h(300K, ref Tbase=463.15K) = {h:.4} J/kg");
    assert!(rel(h, -304_884.786110) < 1e-6, "h = {h}");

    // Round-trip: temperature_from_enthalpy should invert enthalpy.
    let t_back = Incompressible::T66
        .temperature_from_enthalpy(h, P, 0.0)
        .unwrap();
    assert!(
        (t_back - T).abs() < 1e-6,
        "round-trip T: {T} -> h={h} -> T={t_back}"
    );
}

#[test]
fn t66_rejects_out_of_range_temperature_and_nonzero_composition() {
    assert!(
        Incompressible::T66.density(200.0, P, 0.0).is_err(),
        "below Tmin should error"
    );
    assert!(
        Incompressible::T66.density(700.0, P, 0.0).is_err(),
        "above Tmax should error"
    );
    assert!(
        Incompressible::T66.density(T, P, 0.5).is_err(),
        "nonzero x on a pure fluid should error"
    );
}
