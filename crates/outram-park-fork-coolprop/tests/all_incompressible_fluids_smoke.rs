//! Smoke test — every wired incompressible fluid evaluates to a physically
//! sane state.
//!
//! ## Methodology
//!
//! All 126 CoolProp incompressible fluids are wired into
//! [`Incompressible`] (see `dev/regen_incompressible_all.py`). This test
//! iterates `Incompressible::ALL` and, for each fluid, evaluates density,
//! heat capacity, and (when the fluid ships one) conductivity and viscosity
//! at the midpoint of its fitted `T` range (and, for solutions, the midpoint
//! of its fitted `x` range), checking each is finite and positive. It also
//! checks `enthalpy`/`temperature_from_enthalpy` round-trip. Density and
//! heat capacity are always present in CoolProp's own data (never `None`);
//! conductivity and viscosity are sometimes `None` — either genuinely absent
//! or an all-zero never-fit placeholder that `dev/gen_incompressible.py`
//! detects and represents as unavailable rather than a wrong `0.0` (see
//! `incompressibles/mod.rs`'s module doc) — those cases are accepted here,
//! not asserted to succeed.
//!
//! This is a coverage/regression smoke test, not an accuracy check — accuracy
//! for the one fluid verified against a hand evaluation of its own JSON fit
//! is `tests/incompressible_t66.rs`. The point here is to catch a fluid whose
//! generated data fails to evaluate at all (NaN/inf/≤0), which would flag a
//! fit-form or range mishandled for that specific fluid.
//!
//! ## Results (2026-07-10)
//!
//! All 126 fluids evaluate to finite, positive density/heat-capacity at their
//! fitted range's midpoint; conductivity and viscosity likewise wherever
//! present (a handful of fluids have neither — see the module doc).

use outram_park_fork_coolprop::incompressibles::{Incompressible, IncompressibleError};

fn expect_optional(name: &str, prop: &str, result: Result<f64, IncompressibleError>) -> bool {
    match result {
        Ok(v) => {
            assert!(v.is_finite() && v > 0.0, "{name}: non-physical {prop} {v}");
            true
        }
        Err(IncompressibleError::PropertyUnavailable) => false,
        Err(e) => panic!("{name}: {prop} errored unexpectedly: {e:?}"),
    }
}

#[test]
fn every_fluid_evaluates_at_its_midrange_state() {
    let mut checked = 0;
    let mut with_conductivity = 0;
    let mut with_viscosity = 0;
    for &f in Incompressible::ALL {
        let data = f.data();
        let t = 0.5 * (data.t_range.0 + data.t_range.1);
        let x = 0.5 * (data.x_range.0 + data.x_range.1);
        let name = f.name();

        let rho = f.density(t, 0.0, x).unwrap_or_else(|e| panic!("{name}: density failed: {e:?}"));
        assert!(rho.is_finite() && rho > 0.0, "{name}: non-physical density {rho} kg/m3");

        let cp = f.heat_capacity(t, 0.0, x).unwrap_or_else(|e| panic!("{name}: heat_capacity failed: {e:?}"));
        assert!(cp.is_finite() && cp > 0.0, "{name}: non-physical c_p {cp} J/(kg.K)");

        if expect_optional(name, "lambda", f.conductivity(t, 0.0, x)) {
            with_conductivity += 1;
        }
        if expect_optional(name, "mu", f.viscosity(t, 0.0, x)) {
            with_viscosity += 1;
        }

        let h = f.enthalpy(t, 0.0, x).unwrap_or_else(|e| panic!("{name}: enthalpy failed: {e:?}"));
        assert!(h.is_finite(), "{name}: non-finite enthalpy {h}");
        let t_back = f.temperature_from_enthalpy(h, 0.0, x).unwrap_or_else(|e| panic!("{name}: T(h) failed: {e:?}"));
        assert!((t_back - t).abs() < 1e-6, "{name}: T(h) round-trip {t} -> h={h} -> T={t_back}");

        checked += 1;
    }
    assert_eq!(checked, Incompressible::ALL.len());
    assert!(checked >= 126, "expected all incompressible fluids wired, only {checked} checked");
    println!("{checked} fluids checked, {with_conductivity} with conductivity, {with_viscosity} with viscosity");
}
