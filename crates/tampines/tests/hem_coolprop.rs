//! # V&V: the CoolProp HEM closure and the `tampines::hem` re-export (gh #902)
//!
//! | test | what it checks | evidence class |
//! |---|---|---|
//! | `coolprop_rho_e_closure_works_inside_the_dome` | decision D4: CoolProp `(ρ, e)` → `p` inside the two-phase dome, by a Brent root over the `(p, h)` flash | round trip + cross-code (IF97) |
//! | `coolprop_and_if97_agree_on_a_single_phase_water_shock_tube` | the same KNP solver closed by two water EOSs (superheated steam) | cross-code, not validation |
//! | `coolprop_closes_compressed_liquid_water_and_matches_if97` | ~~a recorded limitation of the coolprop liquid flash is an error, not a wrong state~~ **CORRECTED 2026-11-13** (gh:#1349): compressed-liquid water is closed and matches IF97 | cross-code |
//! | `coolprop_and_if97_agree_on_a_compressed_liquid_water_shock_tube` | the same KNP solver on compressed liquid, IF97 against CoolProp (restored, gh:#1349) | cross-code, not validation |
//! | `the_reexported_pipe_is_the_steam_tables_pipe` | `tampines::hem::HemPipe` runs the steam-tables solver unchanged; errors convert to `TampinesError` | unit |
//!
//! Methodology and results are also recorded in
//! `crates/tampines-steam-tables/verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`.

use outram_park_fork_coolprop::Fluid;
use tampines::hem::*;
use tampines::TampinesError;
use uom::si::area::square_meter;
use uom::si::available_energy::{joule_per_kilogram, kilojoule_per_kilogram};
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{megapascal, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// **CoolProp `(ρ, e)` inside the dome (decision D4).**
///
/// # Methodology
///
/// Water at 1 MPa, at `h = 800`, `1500` and `2500 kJ/kg` (qualities ≈ 0.02,
/// 0.37, 0.86). For each: close `(p, h)` with `CoolPropHem` (Maxwell VLE inside
/// the dome), take its `(ρ, e)`, and close that again with `state_rho_e`
/// with no pressure hint and with a hint 30 % high. Then compare the same
/// `(p, h)` against IAPWS-IF97.
///
/// **Pass criteria:** the recovered pressure equals 1 MPa to `1e-8`
/// relative, the state is two-phase, and the quality comes back to `1e-8`.
/// The IF97 comparison is printed, not gated (two different water EOSs).
#[test]
fn coolprop_rho_e_closure_works_inside_the_dome() {
    let cp = CoolPropHem {
        fluid: Fluid::Water,
    };
    let p = Pressure::new::<megapascal>(1.0);
    for h_kj in [800.0, 1500.0, 2500.0] {
        let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);
        let s = cp.state_p_h(p, h).expect("CoolProp (p, h) in the dome");
        let x = s.quality.expect("two-phase").get::<ratio>();
        for hint in [None, Some(p * 1.3)] {
            let t0 = std::time::Instant::now();
            let back = cp
                .state_rho_e(s.density, s.specific_internal_energy, hint)
                .expect("CoolProp (rho, e) inside the dome");
            let us = t0.elapsed().as_secs_f64() * 1e6;
            let rel = (back.pressure.get::<pascal>() / 1.0e6 - 1.0).abs();
            let xb = back.quality.expect("still two-phase").get::<ratio>();
            println!(
                "CoolProp h = {h_kj} kJ/kg: x = {x:.5}, rho = {:.4} kg/m3, c_eq = {:.3} m/s; (rho, e) -> p rel err {rel:.2e}, x back {xb:.8} ({us:.0} us, hint {})",
                s.density.get::<kilogram_per_cubic_meter>(),
                s.equilibrium_sound_speed.get::<meter_per_second>(),
                hint.is_some()
            );
            assert!(rel < 1e-8, "pressure round trip {rel:e}");
            assert!((xb - x).abs() < 1e-8, "quality round trip");
        }
        let w = Iapws97Hem.state_p_h(p, h).unwrap();
        println!(
            "   vs IF97: rho {:+.3} %, T {:+.4} K, x {:+.5}, c_eq {:+.2} %",
            100.0 * ((s.density / w.density).get::<ratio>() - 1.0),
            s.temperature.get::<kelvin>() - w.temperature.get::<kelvin>(),
            x - w.quality.unwrap().get::<ratio>(),
            100.0 * ((s.equilibrium_sound_speed / w.equilibrium_sound_speed).get::<ratio>() - 1.0)
        );
    }
}

/// Run a superheated-steam shock tube with closure `eos`; return the cell
/// pressures and the step count.
fn steam_shock_tube(eos: HemEos, wall_friction: bool) -> (Vec<f64>, usize) {
    shock_tube(eos, 500.0, 0.2, 0.1, 5.0e-4, wall_friction)
}

/// Run a water shock tube (1 m, 40 cells, walls, one temperature `t_k` on both
/// sides, pressures `p_left_mpa | p_right_mpa`) to `t_end` seconds with closure
/// `eos`; return the cell pressures and the step count. `wall_friction =
/// false` is the explicit ablation `HemPipe::without_wall_friction` (gh:#1346:
/// IF97 supplies a viscosity, so friction is on by default for it, while the
/// CoolProp closure supplies none and is frictionless; a like-for-like
/// cross-code comparison ablates it on the IF97 side).
fn shock_tube(
    eos: HemEos,
    t_k: f64,
    p_left_mpa: f64,
    p_right_mpa: f64,
    t_end: f64,
    wall_friction: bool,
) -> (Vec<f64>, usize) {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-3),
        40,
    )
    .unwrap();
    let state = |p_mpa: f64| PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<megapascal>(p_mpa),
        temperature: ThermodynamicTemperature::new::<kelvin>(t_k),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    let mut pipe = HemPipe::new(
        geo,
        eos,
        HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(0.5),
            left: state(p_left_mpa),
            right: state(p_right_mpa),
        },
        HemNumerics::default(),
    )
    .unwrap();
    if !wall_friction {
        pipe = pipe.without_wall_friction();
    }
    let r = pipe.run_until(Time::new::<second>(t_end)).unwrap();
    (
        pipe.snapshot()
            .cells
            .iter()
            .map(|c| c.pressure.get::<pascal>())
            .collect(),
        r.steps,
    )
}

/// **Single-phase water (superheated steam) shock tube, IF97 against
/// CoolProp (cross-code, not validation).**
///
/// # Methodology
///
/// 1 m pipe, 40 cells, walls (the default) at both ends, steam at 500 K,
/// 0.2 MPa left and 0.1 MPa right (superheated on both sides:
/// `T_sat(0.2 MPa) = 393 K`), to `t = 0.5 ms` (the waves travel ≈ 0.3 m and
/// stay inside the pipe). The same solver, closed by `HemEos::Iapws97` and by
/// `HemEos::CoolProp(Water)`. The two are different formulations of water
/// (IF97 vs the IAPWS-95 Helmholtz EOS), so they are expected to agree to a
/// fraction of a percent of the 0.1 MPa jump, not exactly.
///
/// **Why steam and not liquid (the plan asked for liquid):**
/// ~~CoolProp cannot close compressed-liquid water in this workspace yet (see
/// `coolprop_refuses_compressed_liquid_water_instead_of_returning_a_wrong_root`).~~
/// **CORRECTED 2026-11-13** (gh:#1349): it can now; the liquid case is
/// `coolprop_and_if97_agree_on_a_compressed_liquid_water_shock_tube`.
///
/// **Pass criteria:**
/// 1. (unchanged since 2026-10-16) `max_cells |p_IF97 − p_CoolProp| ≤ 5 %` of
///    the jump for the DEFAULT IF97 pipe, a sanity bound only;
/// 2. (added 2026-11-13, gh:#1346, registered before the run) the like-for-like
///    comparison, IF97 with `HemPipe::without_wall_friction` against CoolProp,
///    `≤ 0.1 %` of the jump (the 2026-10-16 value was 8.7 Pa = 0.009 %);
/// 3. wall friction raises the IF97 star pressure by 50 to 300 Pa (the moved
///    number, below).
///
/// **The moved number (explained, gh:#1346).** The IF97 star pressure was
/// 0.139940 MPa on 2026-10-16 and 0.140066 MPa from commit 1211cc9de
/// (2026-11-01, gh:#1061): that commit made wall friction a DEFAULT of
/// `HemPipe`, acting whenever the closure supplies a viscosity. IF97 does
/// (`Iapws97Hem::dynamic_viscosity`); the CoolProp closure does not yet
/// (`HemEos::CoolProp` returns `None`), so it is frictionless. The comparison
/// of the default pipes was therefore no longer like for like. Ablated, the
/// IF97 star pressure is 0.139940 MPa again, bit for bit as on 2026-10-16.
/// The IF97 backward-equation fix of 7be8a7d24 (gh:#1346) plays no part: this
/// state is Region 2a, which it does not touch.
#[test]
fn coolprop_and_if97_agree_on_a_single_phase_water_shock_tube() {
    let t0 = std::time::Instant::now();
    let (p_if97, n_if97) = steam_shock_tube(HemEos::Iapws97, true);
    let t_if97 = t0.elapsed().as_secs_f64();
    let (p_abl, n_abl) = steam_shock_tube(HemEos::Iapws97, false);
    let t1 = std::time::Instant::now();
    let (p_cp, n_cp) = steam_shock_tube(HemEos::CoolProp(Fluid::Water), true);
    let t_cp = t1.elapsed().as_secs_f64();
    let max_abs_diff = |a: &[f64], b: &[f64]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    };
    let max_diff = max_abs_diff(&p_if97, &p_cp);
    let max_diff_abl = max_abs_diff(&p_abl, &p_cp);
    println!(
        "steam shock tube: IF97 {n_if97} steps ({t_if97:.2} s), IF97 no friction {n_abl} steps, CoolProp {n_cp} steps ({t_cp:.2} s); star p IF97 {:.6} MPa, IF97 no friction {:.6} MPa, CoolProp {:.6} MPa; max |dp| default {:.1} Pa = {:.3} % of the jump, no friction {:.1} Pa = {:.3} %",
        p_if97[20] / 1e6,
        p_abl[20] / 1e6,
        p_cp[20] / 1e6,
        max_diff,
        100.0 * max_diff / 0.1e6,
        max_diff_abl,
        100.0 * max_diff_abl / 0.1e6
    );
    assert!(max_diff <= 0.05 * 0.1e6);
    assert!(max_diff_abl <= 0.001 * 0.1e6);
    let friction_shift = p_if97[20] - p_abl[20];
    assert!(
        (50.0..300.0).contains(&friction_shift),
        "wall friction shifts the star pressure by {friction_shift} Pa"
    );
}

/// **CoolProp compressed-liquid water is closed and matches IF97 (gh:#1349).**
///
/// ~~**CoolProp compressed-liquid water is refused, not mis-closed (a recorded
/// limitation).** Measured 2026-10-16: `state_pt(350 K, 2 MPa)` for water
/// returned `ρ = 322.0 kg/m³` with `h ≈ −6e18 J/kg`, and `state_pt(455 K,
/// 1.3 MPa)` a metastable vapour, because `density_pt` started from the
/// ideal-gas density; the closure refused both with `EquationOfState`. If this
/// starts passing a state through, the coolprop flash was fixed and the liquid
/// shock tube can be restored.~~ **CORRECTED 2026-11-13** — gh:#1349 fixed
/// `density_pt` (branch chosen by the Maxwell saturation pressure, dense root
/// solved from the ancillary liquid density, as CoolProp does). The old test
/// asserted that a recorded defect was present; it fails because the defect is
/// gone, so its expectation is replaced and no tolerance is relaxed.
///
/// # Methodology
///
/// Water, `CoolPropHem::state_p_t` at `(2 MPa, 350 K)` and `(1.3 MPa, 455 K)`
/// (`p_sat(455 K)` is about 1.0 MPa, so compressed liquid), against
/// `Iapws97Hem::state_p_t` and, for the first, against CoolProp's own value
/// (974.573279182652 kg/m³, `coolprop_differential/RESULTS.md` defect 1).
/// `HemPipe::new` with `HemEos::CoolProp(Water)` on the first state must
/// succeed.
///
/// **Pass criteria (registered before the run in
/// `verification_and_validation/hem_coolprop_liquid_water_gh1349.md`):**
/// single phase (`quality == None`); `ρ` within 1e-9 of the CoolProp value;
/// against IF97 `|Δρ/ρ| ≤ 1e-4` and `|Δh| ≤ 0.5 kJ/kg`. The IF97 bounds are a
/// few times the order of the IF97 Region 1 vs IAPWS-95 consistency (about
/// 3e-5 in density, 0.2 kJ/kg in enthalpy, recalled, **not re-checked**
/// against the IAPWS R7-97 table). Measured values are in that record.
#[test]
fn coolprop_closes_compressed_liquid_water_and_matches_if97() {
    let cp = CoolPropHem {
        fluid: Fluid::Water,
    };
    let t_of = |k: f64| ThermodynamicTemperature::new::<kelvin>(k);
    for (p_mpa, t_k, rho_ref) in [(2.0, 350.0, Some(974.573279182652)), (1.3, 455.0, None)] {
        let p = Pressure::new::<megapascal>(p_mpa);
        let s = cp
            .state_p_t(p, t_of(t_k))
            .unwrap_or_else(|e| panic!("CoolProp water ({p_mpa} MPa, {t_k} K): {e:?}"));
        let w = Iapws97Hem.state_p_t(p, t_of(t_k)).unwrap();
        let rho = s.density.get::<kilogram_per_cubic_meter>();
        let rho_if97 = w.density.get::<kilogram_per_cubic_meter>();
        let d_rho = rho / rho_if97 - 1.0;
        let d_h = (s.specific_enthalpy - w.specific_enthalpy).get::<joule_per_kilogram>();
        println!(
            "water ({p_mpa} MPa, {t_k} K): CoolProp rho {rho:.9}, IF97 {rho_if97:.9}; d rho/rho {d_rho:+.3e}, d h {d_h:+.2} J/kg, T {:.6} K, c {:.3} vs {:.3} m/s",
            s.temperature.get::<kelvin>(),
            s.equilibrium_sound_speed.get::<meter_per_second>(),
            w.equilibrium_sound_speed.get::<meter_per_second>()
        );
        assert!(s.quality.is_none(), "single phase liquid");
        assert!(rho > 800.0, "liquid, not the vapour root: rho = {rho}");
        if let Some(r) = rho_ref {
            assert!((rho / r - 1.0).abs() < 1e-9, "rho {rho} vs CoolProp {r}");
        }
        assert!(d_rho.abs() <= 1e-4, "density vs IF97 {d_rho:e}");
        assert!(d_h.abs() <= 500.0, "enthalpy vs IF97 {d_h} J/kg");
        assert!((s.temperature.get::<kelvin>() - t_k).abs() < 1e-6);
    }
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-3),
        4,
    )
    .unwrap();
    let pipe = HemPipe::new(
        geo,
        HemEos::CoolProp(Fluid::Water),
        HemInitialCondition::Uniform(PrimitiveState::PressureTemperature {
            pressure: Pressure::new::<megapascal>(2.0),
            temperature: t_of(350.0),
            velocity: Velocity::new::<meter_per_second>(0.0),
        }),
        HemNumerics::default(),
    );
    assert!(pipe.is_ok(), "HemPipe::new on liquid water: {:?}", pipe.err());
}

/// **Compressed-liquid water shock tube, IF97 against CoolProp (cross-code,
/// not validation; restored 2026-11-13, gh:#1349).**
///
/// # Methodology
///
/// 1 m pipe, 40 cells, walls at both ends, water at 350 K on both sides,
/// 2 MPa left and 1 MPa right (liquid throughout: `p_sat(350 K)` = 41.7 kPa),
/// to `t = 0.2 ms` (the waves travel about 0.3 m and stay inside the pipe).
/// The same solver closed by `HemEos::Iapws97` and by `HemEos::CoolProp(Water)`.
///
/// **Pass criteria** (registered before the run): `max_cells |p_IF97 −
/// p_CoolProp| ≤ 1 %` of the 1 MPa jump, a sanity bound between two different
/// formulations of water (stricter than the steam sibling's 5 %); step counts
/// within 10 %. The measured value is printed and recorded in
/// `verification_and_validation/hem_coolprop_liquid_water_gh1349.md`.
#[test]
fn coolprop_and_if97_agree_on_a_compressed_liquid_water_shock_tube() {
    let t0 = std::time::Instant::now();
    // Like for like (gh:#1346): IF97 supplies a viscosity and the CoolProp
    // closure does not, so the IF97 pipe is ablated of wall friction.
    let (p_if97, n_if97) = shock_tube(HemEos::Iapws97, 350.0, 2.0, 1.0, 2.0e-4, false);
    let t_if97 = t0.elapsed().as_secs_f64();
    let (p_fr, _) = shock_tube(HemEos::Iapws97, 350.0, 2.0, 1.0, 2.0e-4, true);
    let t1 = std::time::Instant::now();
    let (p_cp, n_cp) = shock_tube(HemEos::CoolProp(Fluid::Water), 350.0, 2.0, 1.0, 2.0e-4, true);
    let t_cp = t1.elapsed().as_secs_f64();
    let max_diff = p_if97
        .iter()
        .zip(&p_cp)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!(
        "liquid shock tube: IF97 (no friction) {n_if97} steps ({t_if97:.2} s), CoolProp {n_cp} steps ({t_cp:.2} s); star p IF97 no friction {:.6} MPa, IF97 default (friction) {:.6} MPa, CoolProp {:.6} MPa; max |dp| (no friction) = {:.1} Pa = {:.4} % of the jump",
        p_if97[20] / 1e6,
        p_fr[20] / 1e6,
        p_cp[20] / 1e6,
        max_diff,
        100.0 * max_diff / 1.0e6
    );
    assert!(
        (n_if97 as f64 - n_cp as f64).abs() <= 0.1 * n_if97 as f64,
        "step counts {n_if97} vs {n_cp}"
    );
    assert!(max_diff <= 0.01 * 1.0e6);
}

/// **The re-export is the steam-tables solver, unchanged.**
///
/// `tampines::hem::HemPipe` (closure `HemEos::Iapws97`) and
/// `tampines_steam_tables::hem::HemPipe` (closure `Iapws97Hem`) run the same
/// 20 steps of a subcooled/two-phase Riemann problem and must give
/// bit-identical states; `HemEos::IdealGas` must match `IdealGasHem` the same
/// way. A `HemError` must convert into `TampinesError::Hem` with `?`.
#[test]
fn the_reexported_pipe_is_the_steam_tables_pipe() {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-3),
        20,
    )
    .unwrap();
    let ic = HemInitialCondition::Riemann {
        diaphragm: Length::new::<meter>(0.5),
        left: PrimitiveState::PressureTemperature {
            pressure: Pressure::new::<megapascal>(2.0),
            temperature: ThermodynamicTemperature::new::<kelvin>(450.0),
            velocity: Velocity::new::<meter_per_second>(0.0),
        },
        right: PrimitiveState::PressureEnthalpy {
            pressure: Pressure::new::<megapascal>(1.0),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(1.5e6),
            velocity: Velocity::new::<meter_per_second>(0.0),
        },
    };
    let mut a = HemPipe::new(geo, HemEos::Iapws97, ic.clone(), HemNumerics::default()).unwrap();
    let mut b = HemPipeOf::new(geo, Iapws97Hem, ic, HemNumerics::default()).unwrap();
    for _ in 0..20 {
        a.step().unwrap();
        b.step().unwrap();
    }
    assert_eq!(a.snapshot(), b.snapshot());

    let gas = |p: f64, rho: f64| PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    let ic = HemInitialCondition::Riemann {
        diaphragm: Length::new::<meter>(0.5),
        left: gas(1e5, 1.0),
        right: gas(1e4, 0.125),
    };
    let mut c = HemPipe::new(
        geo,
        HemEos::IdealGas(IdealGasHem::air()),
        ic.clone(),
        HemNumerics::default(),
    )
    .unwrap();
    let mut d = HemPipeOf::new(geo, IdealGasHem::air(), ic, HemNumerics::default()).unwrap();
    c.run_until(Time::new::<second>(1e-3)).unwrap();
    d.run_until(Time::new::<second>(1e-3)).unwrap();
    assert_eq!(c.snapshot(), d.snapshot());
    assert_eq!(HemEos::IdealGas(IdealGasHem::air()).name(), "ideal gas");
    assert_eq!(HemEos::CoolProp(Fluid::Water).name(), "CoolProp");

    // Errors cross into TampinesError with `?`, keeping the HemError whole.
    fn break_on_coolprop() -> Result<(), TampinesError> {
        let geo = HemPipeGeometry::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1e-3),
            4,
        )?;
        let mut p = HemPipe::new(
            geo,
            HemEos::CoolProp(Fluid::Water),
            HemInitialCondition::Uniform(PrimitiveState::PressureTemperature {
                pressure: Pressure::new::<megapascal>(0.2),
                temperature: ThermodynamicTemperature::new::<kelvin>(500.0),
                velocity: Velocity::new::<meter_per_second>(0.0),
            }),
            HemNumerics::default(),
        )?;
        p.set_boundary(
            PipeEnd::Outlet,
            HemBoundary::ChokedBreak {
                area_fraction: Ratio::new::<ratio>(1.0),
                back_pressure: Pressure::new::<megapascal>(0.1),
            },
        )?;
        p.step()?;
        Ok(())
    }
    match break_on_coolprop() {
        Err(TampinesError::Hem(HemError::Unsupported { eos, .. })) => assert_eq!(eos, "CoolProp"),
        other => panic!("expected TampinesError::Hem(Unsupported), got {other:?}"),
    }
    assert!(matches!(
        CoolPropHem {
            fluid: Fluid::Water
        }
        .state_p_rho(
            Pressure::new::<pascal>(1e5),
            MassDensity::new::<kilogram_per_cubic_meter>(1.0)
        ),
        Err(HemError::Unsupported { .. })
    ));
}
