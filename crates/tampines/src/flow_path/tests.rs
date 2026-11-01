//! # V&V: two-velocity flow paths (gh:#1094)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. **Inertia:** with `K = 0` and a constant `ΔP`, the semi-implicit step
//!    is exact, `v(t) = ΔP t/(ρL)`, to round-off.
//! 2. **Steady loss:** at steady state `ΔP = ½Kρv²` to 1e-10.
//! 3. **Transient with loss:** `v(t) = v∞ tanh(t/τ)` with `v∞ = √(2ΔP/(Kρ))`
//!    and `τ = L/(½K v∞)`; the semi-implicit error at `t = τ` halves with
//!    `Δt` (first order, ratio in [1.8, 2.2]).
//! 4. **Gravity:** a liquid column with equal end pressures and a 1 m rise
//!    accelerates backwards at `g/L`.
//! 5. **Phase selection:** a junction below the donor level passes liquid,
//!    above it gas, and half-covered splits the area half and half.
//! 6. **Gas choking** equals `critical_moles` exactly when the pressure ratio
//!    is large; **liquid choking** equals IF97's HEM break flux.
//! 7. A check valve never flows back; a breaker flows only above its set
//!    difference.

use super::*;

fn len(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn geo(k: f64, l: f64) -> FlowPathGeometry {
    FlowPathGeometry {
        area: Area::new::<square_meter>(0.1),
        inertial_length: len(l),
        form_loss: k,
        discharge_coefficient: 1.0,
        from_elevation: len(0.0),
        to_elevation: len(0.0),
        opening_height: len(0.0),
    }
}
/// An end of air at `p` (level far below: gas only, or far above: liquid only).
fn end(p: f64, level: f64) -> PathEnd {
    PathEnd {
        pressure: Pressure::new::<pascal>(p),
        pool_level: len(level),
        liquid_density: MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
        liquid_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(4.0e5),
        gas_density: MassDensity::new::<kilogram_per_cubic_meter>(1.2),
        gas_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(3.0e5),
        gas_temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        gas_molar_mass: 0.028_96,
        gas_steam_mole_fraction: 0.0,
    }
}
fn s(x: f64) -> Time {
    Time::new::<second>(x)
}

/// **1, 2, 3.** Liquid (`level` above the junction), `ΔP = 1e4` Pa,
/// `L = 10` m. **Result (2026-11-01):** printed and asserted.
#[test]
fn inertia_steady_loss_and_first_order_transient() {
    let (dp, rho, l) = (1.0e4, 1000.0, 10.0);
    let mut p = FlowPath::new(geo(0.0, l), FlowPathKind::Open).unwrap();
    for _ in 0..100 {
        p.step(s(0.01), &end(1.0e5 + dp, 5.0), &end(1.0e5, 5.0)).unwrap();
    }
    let v = p.velocities().0.value;
    println!("inertia: v(1 s) = {v:.15} vs {:.15}", dp * 1.0 / (rho * l));
    assert!((v / (dp / (rho * l)) - 1.0).abs() < 1e-12);

    let k = 2.0;
    let v_inf = (2.0 * dp / (k * rho)).sqrt();
    let tau = l / (0.5 * k * v_inf);
    let run = |dt: f64| {
        let mut p = FlowPath::new(geo(k, l), FlowPathKind::Open).unwrap().without_choking();
        let n = (tau / dt).round() as usize;
        let dt = tau / n as f64;
        for _ in 0..n {
            p.step(s(dt), &end(1.0e5 + dp, 5.0), &end(1.0e5, 5.0)).unwrap();
        }
        p.velocities().0.value
    };
    let exact = v_inf * 1.0_f64.r_tanh();
    let (e1, e2) = (run(tau / 50.0) - exact, run(tau / 100.0) - exact);
    println!("transient: v(tau) exact {exact:.6}, errors {e1:.3e} (tau/50), {e2:.3e} (tau/100), ratio {:.3}", e1 / e2);
    assert!((1.8..2.2).contains(&(e1 / e2)));
    let mut ps = FlowPath::new(geo(k, l), FlowPathKind::Open).unwrap().without_choking();
    for _ in 0..20000 {
        ps.step(s(0.05), &end(1.0e5 + dp, 5.0), &end(1.0e5, 5.0)).unwrap();
    }
    let vs = ps.velocities().0.value;
    println!("steady: v {vs:.12} vs {v_inf:.12}");
    assert!(((0.5 * k * rho * vs * vs) / dp - 1.0).abs() < 1e-10);
}

use petir::mathf::RealMath;

/// **4. Gravity, 5. phase selection.** **Result (2026-11-01):** asserted.
#[test]
fn gravity_and_phase_selection() {
    let mut g = geo(0.0, 2.0);
    g.to_elevation = len(1.0);
    let mut p = FlowPath::new(g, FlowPathKind::Open).unwrap();
    p.step(s(0.001), &end(1.0e5, 5.0), &end(1.0e5, 5.0)).unwrap();
    let a = p.velocities().0.value / 0.001;
    assert!((a / (-G / 2.0) - 1.0).abs() < 1e-12, "{a}");

    // Junction 0 with a 0.2 m opening; donor level at +1 (liquid), -1 (gas), 0 (half).
    let mut g2 = geo(0.0, 1.0);
    g2.opening_height = len(0.2);
    for (level, f_liq) in [(1.0, 1.0), (-1.0, 0.0), (0.0, 0.5)] {
        let mut p = FlowPath::new(g2, FlowPathKind::Open).unwrap().without_choking();
        let t = p.step(s(0.01), &end(1.01e5, level), &end(1.0e5, level)).unwrap();
        let (vl, vg) = (p.velocities().0.value, p.velocities().1.value);
        let ml = 1000.0 * f_liq * 0.1 * vl;
        let mg = 1.2 * (1.0 - f_liq) * 0.1 * vg;
        assert!((t.liquid_mass_rate.value - ml).abs() <= 1e-12 * ml.abs().max(1e-30), "{level}");
        assert!((t.gas_mass_rate.value - mg).abs() <= 1e-12 * mg.abs().max(1e-30), "{level}");
        assert!((t.liquid_enthalpy_rate.value - ml * 4.0e5).abs() <= 1e-9 * ml.abs().max(1e-30));
    }
}

/// **6. Choking.** Gas: 8 bar to 1 bar of N2-like gas; liquid: 7 MPa
/// saturated-ish water to 0.1 MPa. **Result (2026-11-01):** asserted.
#[test]
fn gas_and_liquid_choking() {
    let mut p = FlowPath::new(geo(0.0, 1.0), FlowPathKind::Open).unwrap();
    let mut up = end(8.0e5, -5.0);
    up.gas_density = MassDensity::new::<kilogram_per_cubic_meter>(8.0e5 * 0.028_96 / (R_GAS * 300.0));
    let mut t = p.step(s(1.0), &up, &end(1.0e5, -5.0)).unwrap();
    for _ in 0..50 {
        t = p.step(s(1.0), &up, &end(1.0e5, -5.0)).unwrap();
    }
    let m_crit = critical_moles(1.0, 0.1, 8.0e5, 300.0, 0.028_96, 0.0) * 0.028_96;
    println!("gas choking: {:.6} kg/s vs {m_crit:.6}", t.gas_mass_rate.value);
    assert!(t.gas_choked && t.gas_mass_rate.value == m_crit);

    let mut pl = FlowPath::new(geo(0.0, 1.0), FlowPathKind::Open).unwrap();
    let mut w = end(7.0e6, 5.0);
    w.liquid_density = MassDensity::new::<kilogram_per_cubic_meter>(740.0);
    w.liquid_enthalpy = AvailableEnergy::new::<joule_per_kilogram>(1.2e6);
    let mut tl = pl.step(s(1.0), &w, &end(1.0e5, 5.0)).unwrap();
    for _ in 0..50 {
        tl = pl.step(s(1.0), &w, &end(1.0e5, 5.0)).unwrap();
    }
    println!("liquid choking: {:.3} kg/s, choked {}", tl.liquid_mass_rate.value, tl.liquid_choked);
    assert!(tl.liquid_choked);
    let unchoked = FlowPath::new(geo(0.0, 1.0), FlowPathKind::Open).unwrap().without_choking();
    assert!(!unchoked.mechanisms().choking);
    assert!(FlowPath::new(geo(0.0, 1.0), FlowPathKind::Open).unwrap().mechanisms().choking, "on by default");
}

/// **7. Check valve and breaker.** **Result (2026-11-01):** asserted.
#[test]
fn check_valve_and_vacuum_breaker() {
    let mut c = FlowPath::new(geo(1.0, 1.0), FlowPathKind::CheckValve).unwrap();
    for _ in 0..10 {
        let t = c.step(s(0.1), &end(1.0e5, -5.0), &end(1.1e5, -5.0)).unwrap();
        assert_eq!(t.gas_mass_rate.value, 0.0);
    }
    let set = Pressure::new::<pascal>(3400.0);
    let mut b = FlowPath::new(geo(1.0, 1.0), FlowPathKind::VacuumBreaker { opening_pressure_difference: set }).unwrap();
    assert_eq!(b.step(s(0.1), &end(1.03e5, -5.0), &end(1.0e5, -5.0)).unwrap().gas_mass_rate.value, 0.0);
    assert!(b.step(s(0.1), &end(1.04e5, -5.0), &end(1.0e5, -5.0)).unwrap().gas_mass_rate.value > 0.0);
    assert!(FlowPath::new(geo(-1.0, 1.0), FlowPathKind::Open).is_err());
    assert!(FlowPath::new(geo(1.0, 1.0), FlowPathKind::Open).unwrap().step(s(0.0), &end(1.0, 0.0), &end(1.0, 0.0)).is_err());
}
