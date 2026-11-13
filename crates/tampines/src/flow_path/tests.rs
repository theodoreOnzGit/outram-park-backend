//! # V&V: two-velocity flow paths (gh:#1094)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. **Inertia:** with `K = 0` and a constant `ΔP`, the semi-implicit step
//!    is exact, `v(t) = ΔP t/(ρL)`, to round-off.
//! 2. **Steady loss:** at steady state `ΔP = ½Kρv²` to 1e-10.
//! 3. **Transient with loss:** `v(t) = v∞ tanh(t/τ)` with `v∞ = √(2ΔP/(Kρ))`
//!    and `τ = L/(½K v∞)`; the semi-implicit error at `t = τ` halves with
//!    `Δt` (first order, ratio in [1.8, 2.2]). **MISSED (2026-11-01), in the
//!    good direction:** the ratio is 4.000 (errors 1.771e-4 and 4.427e-5 at
//!    τ/50 and τ/100), second order at this point. ~~Not explained: the
//!    lagged-loss backward-Euler step is first order in general, so this may
//!    be cancellation at `t = τ`; a check at other times is filed.~~
//!    **CORRECTED 2026-11-01 (gh:#1098 item 1): explained, and not
//!    cancellation.** For a constant `ΔP` the lagged quadratic loss is
//!    second order at every time; on this case the step is the `tanh`
//!    addition formula and reproduces `v∞ tanh(n artanh(Δt/τ))` exactly
//!    (case 3b below, measured ratio 4.00 at τ/2, τ and 2τ). The scheme is
//!    first order only when `ΔP` varies within a step and is held at its
//!    start value, as in a network (case 3b's ramp: ratio 2.000). The
//!    assertion here still only requires the error to fall with `Δt` (chosen
//!    after the run, and labelled so); case 3b holds the tight bar.
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
    // Prediction was [1.8, 2.2]; measured 4.0 (see the module docs). Only
    // the direction is asserted, a bar set after the run.
    assert!(e2.abs() < e1.abs());
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
/// saturated-ish water to 0.1 MPa. **Result (2026-11-01):** gas 186.650459
/// kg/s = `critical_moles` exactly; liquid choked. (The liquid case first
/// failed on a defect: the stagnation enthalpy used the pre-limit velocity.)
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

/// **3b (gh:#1098 item 1). Why case 3 is second order: an exact identity,
/// not cancellation at `t = τ`.**
///
/// **Derivation (written before the run, 2026-11-01).** With `u = v/v∞`,
/// `s = t/τ` and `h = Δt/τ`, the momentum equation is the constant-
/// coefficient Riccati equation `du/ds = 1 − u²`, and the lagged-loss step
/// `(ρL/Δt + ½Kρ|v^n|) v^{n+1} = ρL/Δt v^n + ΔP` becomes (for `u ≥ 0`)
///
/// ```text
/// u_{n+1} = (u_n + h) / (1 + h u_n).
/// ```
///
/// That is the addition formula `tanh(a + b) = (tanh a + tanh b)/(1 + tanh a
/// tanh b)` with `tanh b` replaced by `h`. From `u_0 = 0` the step is
/// therefore EXACT on a stretched clock:
///
/// ```text
/// u_n = tanh(n · artanh h) = tanh(s · artanh(h)/h),
/// artanh(h)/h = 1 + h²/3 + h⁴/5 + …,
/// u_n − tanh s = sech²(s) · s h²/3 + O(h⁴).
/// ```
///
/// The leading error is `h²`-proportional at **every** `s`, with a
/// coefficient `s sech² s / 3` that vanishes nowhere on `s > 0`; the ratio
/// is `4 + O(h²)` at all times. More generally, a Taylor expansion of
/// `u_{n+1} = (u + ha)/(1 + hbu)` gives `u + h(a − bu²) + h²(b²u³ − abu) +
/// O(h³)`, which matches the exact `h u' + h²u''/2` for `u' = a − bu²` with
/// `a`, `b` constant: the linearised quadratic loss is second order for
/// any constant `ΔP`. The scheme falls to first order only when `ΔP`
/// changes within a step and is held at its start-of-step value — which is
/// what a network does (gh:#1095), so the module's "first order" holds for
/// the coupled problem, not for a path with fixed ends.
///
/// **Predictions:** (a) the path's `v_n` equals `v∞ tanh(n artanh h)` to
/// 1e-12 relative at `s = ½, 1, 2`, for `h = 1/50, 1/100`; (b) the error
/// against `v∞ tanh s` matches `v∞ sech²(s) s h²/3` to 2 % at `h = 1/100`;
/// (c) error ratios within `[3.9, 4.1]` at all three times; (d) with `ΔP(t)
/// = ΔP₀ t/τ` held at each step's start, the self-convergence ratio
/// `(v_h − v_{h/2})/(v_{h/2} − v_{h/4})` at `s = 1` is in `[1.8, 2.2]`
/// (first order).
///
/// **Result (2026-11-01, release):** all four held. Errors at `h = 1/50`,
/// `1/100` and their ratio: `s = ½` 1.658e-4, 4.145e-5, 4.001; `s = 1`
/// 1.771e-4, 4.427e-5, 4.000; `s = 2` 5.958e-5, 1.489e-5, 4.000 (the
/// leading term `v∞ s sech²(s) h²/3` gives 1.658e-4, 1.771e-4, 5.959e-5 at
/// `h = 1/50`). Identity (a) holds to 3.8e-15 relative worst. Ramp
/// self-convergence ratio 2.000 (first order). See
/// `verification_and_validation/flow_path.md`.
#[test]
fn transient_second_order_is_a_tanh_addition_identity() {
    let (dp, rho, l, k): (f64, f64, f64, f64) = (1.0e4, 1000.0, 10.0, 2.0);
    let v_inf = (2.0 * dp / (k * rho)).sqrt();
    let tau = l / (0.5 * k * v_inf);
    let atanh = |x: f64| 0.5 * ((1.0 + x) / (1.0 - x)).r_ln();
    let sech2 = |x: f64| 1.0 / (x.r_cosh() * x.r_cosh());
    let mut worst_identity: f64 = 0.0;
    for s_end in [0.5, 1.0, 2.0] {
        let mut errs = [0.0; 2];
        for (i, per_tau) in [50usize, 100].into_iter().enumerate() {
            let n = (s_end * per_tau as f64).round() as usize;
            let h = 1.0 / per_tau as f64;
            let mut p = FlowPath::new(geo(k, l), FlowPathKind::Open).unwrap().without_choking();
            for _ in 0..n {
                p.step(s(h * tau), &end(1.0e5 + dp, 5.0), &end(1.0e5, 5.0)).unwrap();
            }
            let v = p.velocities().0.value;
            let identity = v_inf * (n as f64 * atanh(h)).r_tanh();
            worst_identity = worst_identity.max((v / identity - 1.0).abs());
            assert!((v / identity - 1.0).abs() < 1e-12, "identity at s {s_end}, h {h}: {v} vs {identity}");
            errs[i] = v - v_inf * s_end.r_tanh();
            if per_tau == 100 {
                let lead = v_inf * sech2(s_end) * s_end * h * h / 3.0;
                assert!((errs[i] / lead - 1.0).abs() < 0.02, "leading term at s {s_end}: {} vs {lead}", errs[i]);
            }
        }
        let ratio = errs[0] / errs[1];
        println!("s = {s_end}: errors {:.3e} (h=1/50), {:.3e} (h=1/100), ratio {ratio:.3}", errs[0], errs[1]);
        assert!((3.9..=4.1).contains(&ratio), "s {s_end}: ratio {ratio}");
    }
    println!("identity worst relative {worst_identity:.2e}");

    // (d) ΔP ramp held at the start of each step: first order.
    let ramp = |per_tau: usize| {
        let h = 1.0 / per_tau as f64;
        let mut p = FlowPath::new(geo(k, l), FlowPathKind::Open).unwrap().without_choking();
        for i in 0..per_tau {
            let dpi = dp * (i as f64 * h);
            p.step(s(h * tau), &end(1.0e5 + dpi, 5.0), &end(1.0e5, 5.0)).unwrap();
        }
        p.velocities().0.value
    };
    let (a, b, c) = (ramp(50), ramp(100), ramp(200));
    let r = (a - b) / (b - c);
    println!("ramp (held ΔP): v(τ) {a:.9} {b:.9} {c:.9}, self-convergence ratio {r:.3}");
    assert!((1.8..=2.2).contains(&r), "ramp ratio {r}");
}
