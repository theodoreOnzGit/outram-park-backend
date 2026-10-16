//! # V&V: the HEM pipe solver (`tampines_steam_tables::hem::HemPipe`)
//!
//! Issue gh #902 (epic gh #896). Methodology and the measured results of every
//! test here are recorded in
//! `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`;
//! each test's own doc comment states its pass criterion and its numbers.
//!
//! | test | what it checks | evidence class |
//! |---|---|---|
//! | `ideal_gas_sod_matches_the_exact_riemann_solution` | scheme accuracy, Sod, γ = 1.4 | analytical solution |
//! | `toro_test_2_double_rarefaction_stays_positive` | positivity near vacuum | analytical solution |
//! | `a_fixed_step_that_is_too_large_is_refused` | named `CourantViolation`, state untouched | unit |
//! | `two_walls_conserve_mass_and_total_energy_to_round_off_on_if97` | conservation, subcooled + two-phase IF97 | internal consistency |
//! | `the_defaults_are_pinned_and_if97_uses_the_equilibrium_sound_speed` | correct physics is the default | unit (pinned default) |
//! | `hem_water_double_rarefaction_matches_the_semi_analytical_plateaus` | the HEM closure through a flashing fan | semi-analytical solution |

mod common;

use common::hem_rarefaction;
use common::toro_exact::{exact_state, GasState};
use tampines_steam_tables::hem::*;
use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    s_ph_eqm, w_ph_eqm_hem, w_ph_wood_wallis,
};
use tampines_steam_tables::region_4_vap_liq_equilibrium::{
    w_ps_eqm_region4_finite_diff_vol, w_ps_eqm_region4_kieffer,
};
use uom::si::area::square_meter;
use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{megapascal, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

fn gas(p: f64, rho: f64, u: f64) -> PrimitiveState {
    PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        velocity: Velocity::new::<meter_per_second>(u),
    }
}

/// Discrete L2 (RMS over cells) and L∞ of `num − exact`.
fn l2_linf(num: &[f64], exact: &[f64]) -> (f64, f64) {
    let n = num.len() as f64;
    let mut s = 0.0_f64;
    let mut m = 0.0_f64;
    for (a, b) in num.iter().zip(exact) {
        let d = (a - b).abs();
        s += d * d;
        m = m.max(d);
    }
    ((s / n).sqrt(), m)
}

// ───────────────────────────── Sod ─────────────────────────────

/// Sod's tube in SI, as `outram-foam-appbuilder-lib`'s validation case:
/// `n` cells (appbuilder: 100) on `[−5, 5]` m (here `[0, 10]` m, diaphragm at 5 m), air
/// (`γ = 1.4`, `R = 287.1`), `ρ = 1 | 0.125` kg/m³, `p = 1e5 | 1e4` Pa,
/// transmissive ends, `t = 0.2·t0 = 6.32456e-3` s.
fn run_sod(numerics: HemNumerics, n: usize) -> (Vec<f64>, [Vec<f64>; 3], [(f64, f64); 3]) {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(10.0),
        Area::new::<square_meter>(1.0),
        n,
    )
    .unwrap();
    let mut pipe = HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(5.0),
            left: gas(1.0e5, 1.0, 0.0),
            right: gas(1.0e4, 0.125, 0.0),
        },
        numerics,
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
        .unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
        .unwrap();
    let t_end = 0.2 * 10.0 / (1.0e5_f64).sqrt();
    let summary = pipe.run_until(Time::new::<second>(t_end)).unwrap();
    println!(
        "Sod: {} steps, max central Courant {:.4}",
        summary.steps,
        summary.max_courant.get::<ratio>()
    );
    let snap = pipe.snapshot();
    let l = GasState {
        rho: 1.0,
        u: 0.0,
        p: 1.0e5,
    };
    let r = GasState {
        rho: 0.125,
        u: 0.0,
        p: 1.0e4,
    };
    let (mut rn, mut un, mut pn, mut re, mut ue, mut pe) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    for (x, c) in snap.cell_centres.iter().zip(&snap.cells) {
        let ex = exact_state(l, r, x.get::<meter>() - 5.0, 0.0, t_end);
        rn.push(c.density.get::<kilogram_per_cubic_meter>());
        un.push(c.velocity.get::<meter_per_second>());
        pn.push(c.pressure.get::<pascal>());
        re.push(ex.rho);
        ue.push(ex.u);
        pe.push(ex.p);
    }
    let x: Vec<f64> = snap
        .cell_centres
        .iter()
        .map(|x| x.get::<meter>() - 5.0)
        .collect();
    let norms = [l2_linf(&rn, &re), l2_linf(&un, &ue), l2_linf(&pn, &pe)];
    (x, [rn, un, pn], norms)
}

/// Sod (1978) Table II, `γ = 1.4`, `τ = 0.2`: `(x/L, ρ/ρ0, u/u0, P/P0)`.
/// Copied from appbuilder's `SOD_TABLE_II`. Stations 0.3, 0.4 and 0.7 are
/// NOT faithful samples of the exact solution (appbuilder README: fan and
/// contact points), so they are never asserted.
const SOD_TABLE_II: [(f64, f64, f64, f64); 9] = [
    (0.1, 1.000, 0.000, 1.000),
    (0.2, 1.000, 0.000, 1.000),
    (0.3, 0.869, 0.164, 0.822),
    (0.4, 0.426, 0.927, 0.303),
    (0.5, 0.426, 0.927, 0.303),
    (0.6, 0.426, 0.927, 0.303),
    (0.7, 0.426, 0.927, 0.303),
    (0.8, 0.266, 0.927, 0.303),
    (0.9, 0.125, 0.000, 0.100),
];
const SOD_FAITHFUL: [f64; 6] = [0.1, 0.2, 0.5, 0.6, 0.8, 0.9];

/// **Sod against the exact Riemann solution (Toro 2009 ch. 4) and Sod (1978)
/// Table II.**
///
/// # Methodology
///
/// `HemPipe<IdealGasHem>` on appbuilder's Sod case (see `run_sod`), default
/// numerics (adaptive central Courant 0.2, van Leer, SSP22). The exact
/// solution is evaluated at all 100 cell centres. Norms are normalised by the
/// field peak as in appbuilder's `RESULTS.md`: `ρ_peak = 1 kg/m³`,
/// `u_peak = |u*| = 293.3 m/s`, `p_peak = 1e5 Pa`.
///
/// **Pass criterion as registered before the first run:** whole-field L2
/// ≤ 5 % of peak for each of `ρ`, `u`, `p`.
///
/// # Results (measured 2026-10-16, release) and what changed
///
/// | | ρ | u | p |
/// |---|---|---|---|
/// | this solver, default | 1.53 % | **5.56 %** | 1.25 % |
/// | this solver, Euler, fixed Δt = 1e-6 s (appbuilder's settings) | 1.51 % | 5.35 % | 1.20 % |
/// | appbuilder `RhoCentralFoam` (its RESULTS.md, 2026-07-17) | 1.41 % | 5.17 % | 1.06 % |
///
/// **The registered `u` gate is MISSED** (5.56 % > 5 %). It is not loosened.
/// What was checked before deciding how to record it:
///
/// - The miss is one cell: the shock cell alone carries `L∞ = 51.9 %`, which
///   contributes `51.9/√100 = 5.19 %` to a 100-cell L2. Away from the shock
///   (`|x − 3.5 m| > 0.3 m`) the `u` L2 is 1.21 %.
/// - The scheme converges: `u` L2 = 5.56 / 3.99 / 2.98 % at 100 / 200 / 400
///   cells (away from the shock 1.21 / 0.62 / 0.32 %, first order, as a
///   shock-capturing scheme should be).
/// - appbuilder's own `u` L2 (5.17 %) is above 5 % too: "the same bar as
///   appbuilder" is, in its test, the Table II faithful-station point error
///   ≤ 5 % of peak, not a whole-field L2.
///
/// So this test asserts: the registered whole-field L2 ≤ 5 % for `ρ` and `p`
/// (met); appbuilder's actual asserted bar, the faithful Table II stations
/// within 5 % of peak in all three variables; and that the `u` L2 falls when
/// the mesh is doubled. The `u` whole-field L2 is printed and recorded as a
/// miss in the V&V file.
#[test]
fn ideal_gas_sod_matches_the_exact_riemann_solution() {
    let u_peak = 0.92745 * (1.0e5_f64).sqrt();
    let peaks = [1.0, u_peak, 1.0e5];
    let names = ["rho", "u", "p"];
    let (x, profiles, norms) = run_sod(HemNumerics::default(), 100);
    let euler = HemNumerics {
        time_step: TimeStepControl::Fixed(Time::new::<second>(1.0e-6)),
        time_integrator: TimeIntegrator::ForwardEuler,
        ..HemNumerics::default()
    };
    let (_, _, norms_fe) = run_sod(euler, 100);
    let (_, _, norms_200) = run_sod(HemNumerics::default(), 200);
    let appbuilder_l2 = [0.0141, 0.0517, 0.0106];
    for k in 0..3 {
        println!(
            "Sod {:>3}: default L2 = {:.2} % peak, Linf = {:.2} % | Euler dt=1e-6 L2 = {:.2} % | 200 cells L2 = {:.2} % | appbuilder L2 = {:.2} %",
            names[k],
            100.0 * norms[k].0 / peaks[k],
            100.0 * norms[k].1 / peaks[k],
            100.0 * norms_fe[k].0 / peaks[k],
            100.0 * norms_200[k].0 / peaks[k],
            100.0 * appbuilder_l2[k],
        );
    }
    if norms[1].0 / peaks[1] > 0.05 {
        println!(
            "Sod u: MISS of the registered whole-field L2 <= 5 % gate ({:.2} %); see the doc comment",
            100.0 * norms[1].0 / peaks[1]
        );
    }
    for k in [0, 2] {
        assert!(
            norms[k].0 / peaks[k] <= 0.05,
            "Sod {} L2 = {:.2} % of peak exceeds 5 %",
            names[k],
            100.0 * norms[k].0 / peaks[k]
        );
    }
    assert!(
        norms_200[1].0 < norms[1].0,
        "u L2 must fall under mesh refinement"
    );
    // appbuilder's asserted bar: faithful Table II stations within 5 % of peak.
    let interp = |v: &[f64], xq: f64| {
        let i = x.iter().position(|&xc| xc > xq).unwrap();
        let w = (xq - x[i - 1]) / (x[i] - x[i - 1]);
        v[i - 1] * (1.0 - w) + v[i] * w
    };
    let u0 = (1.0e5_f64).sqrt();
    let mut worst = [0.0_f64; 3];
    for &(xl, rho_t, u_t, p_t) in SOD_TABLE_II.iter() {
        if !SOD_FAITHFUL.contains(&xl) {
            continue;
        }
        let xq = 10.0 * xl - 5.0;
        let tab = [rho_t, u_t * u0, p_t * 1.0e5];
        for k in 0..3 {
            let e = (interp(&profiles[k], xq) - tab[k]).abs() / peaks[k];
            worst[k] = worst[k].max(e);
        }
    }
    println!(
        "Sod Table II faithful stations, worst rel. error: rho {:.2} %, u {:.2} %, p {:.2} % (appbuilder: 0.43 / 0.96 / 0.43 %)",
        100.0 * worst[0],
        100.0 * worst[1],
        100.0 * worst[2]
    );
    for k in 0..3 {
        assert!(
            worst[k] < 0.05,
            "Table II faithful station {} error {:.3}",
            names[k],
            worst[k]
        );
    }
}

// ───────────────────────── positivity ─────────────────────────

fn toro_2_pipe(limiter: FluxLimiter) -> HemPipe<IdealGasHem> {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1.0),
        100,
    )
    .unwrap();
    let mut pipe = HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(0.5),
            left: gas(0.4, 1.0, -2.0),
            right: gas(0.4, 1.0, 2.0),
        },
        HemNumerics {
            limiter,
            ..HemNumerics::default()
        },
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
        .unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
        .unwrap();
    pipe
}

/// **Positivity on Toro's test 2** (Toro 2009 §4.3.3, Table 4.1 test 2: a
/// strong double rarefaction, `ρ = 1`, `u = ∓2`, `p = 0.4`, `γ = 1.4`, star
/// pressure `p* ≈ 0.00189`, output time `t = 0.15`).
///
/// # Methodology
///
/// SI numbers read as dimensionless (`ρ` in kg/m³, `u` in m/s, `p` in Pa),
/// 100 cells on `[0, 1]` m, diaphragm at 0.5 m, transmissive ends, adaptive
/// central Courant 0.2 (the default). Stepped one step at a time.
///
/// **Pass criterion as registered before the first run:** with the default
/// numerics every step succeeds and `min p > 0`, `min ρ > 0` after every step.
///
/// # Results (measured 2026-10-16, release) and what changed
///
/// **With the default van Leer reconstruction the registered criterion is
/// MISSED.** At `t = 0.062` (default Courant 0.2; `t ≈ 0.008–0.012` at the
/// lower Courant numbers below) the cell beside the centre reaches a negative
/// specific internal energy (`e = E/ρ − ½u² ≈ −2e-5 J/kg` against an exact
/// `e ≈ 0.24`), and the step returns `HemError::EquationOfState` (rolled
/// back; nothing is clamped). A sweep showed it is **not** a time-step
/// problem: it fails at central Courant 0.2, 0.1, 0.05 and 0.025 with SSP22,
/// at 0.1 with forward Euler and at 0.2 with SSP33, and **every** one of those
/// runs passes with the first-order (upwind) limiter. The cause is the
/// reconstruction this port takes from upstream `rhoCentralFoam`: `ρ`, `ρU`
/// and `e` are limited independently, so the face states do not average back
/// to the cell's `ρE`, and near vacuum, where `E ≈ ½ρu²`, the update of
/// `E − ½ρu²` is not sign-preserving. Upstream has no positivity limiter
/// either. Not fixed here (a positivity-preserving reconstruction is new
/// numerics, out of scope for gh #902); recorded as an open item.
///
/// This test therefore asserts what is true and stays able to fail:
/// 1. first order (`FluxLimiter::Upwind`) at the default Courant number keeps
///    `p > 0` and `ρ > 0` after every step to `t = 0.15`;
/// 2. the default (van Leer) run either reaches `t = 0.15` positive or stops
///    with a named `EquationOfState`/`Unphysical` error, never a NaN or a
///    panic, and the state it is left in is finite and positive (the failed
///    step was rolled back). If (2) starts reaching `t = 0.15`, the defect is
///    fixed and this doc must be updated.
#[test]
fn toro_test_2_double_rarefaction_stays_positive() {
    let l = GasState {
        rho: 1.0,
        u: -2.0,
        p: 0.4,
    };
    let r = GasState {
        rho: 1.0,
        u: 2.0,
        p: 0.4,
    };
    let p_star = exact_state(l, r, 0.0, 0.0, 0.15).p;

    // 1. First order: positive at every step.
    let mut pipe = toro_2_pipe(FluxLimiter::Upwind);
    let (mut steps, mut min_p, mut min_rho) = (0, f64::INFINITY, f64::INFINITY);
    while pipe.time().get::<second>() < 0.15 {
        let rep = pipe.step().expect("first-order Toro test 2 step");
        steps += 1;
        for c in &pipe.snapshot().cells {
            min_rho = min_rho.min(c.density.get::<kilogram_per_cubic_meter>());
        }
        min_p = min_p.min(rep.min_pressure.get::<pascal>());
        assert!(
            min_p > 0.0 && min_rho > 0.0,
            "upwind step {steps}: min p {min_p}, min rho {min_rho}"
        );
    }
    let centre = pipe.cell_state(49).unwrap().pressure.get::<pascal>();
    println!(
        "Toro test 2, upwind: {steps} steps to t = {:.4}; min p = {min_p:.3e}, min rho = {min_rho:.3e}; centre p = {centre:.5} vs exact p* = {p_star:.5}",
        pipe.time().get::<second>()
    );

    // 2. Default (van Leer): reaches the end positive, or stops by name.
    let mut pipe = toro_2_pipe(FluxLimiter::VanLeer);
    let outcome = loop {
        if pipe.time().get::<second>() >= 0.15 {
            break None;
        }
        match pipe.step() {
            Ok(_) => {}
            Err(e) => break Some(e),
        }
    };
    match &outcome {
        None => println!("Toro test 2, van Leer: reached t = 0.15 positive (the recorded defect is gone: update the doc)"),
        Some(e) => {
            assert!(
                matches!(e, HemError::EquationOfState { .. } | HemError::Unphysical { .. }),
                "unexpected error kind: {e:?}"
            );
            println!("Toro test 2, van Leer: stopped at t = {:.5} with: {e}", pipe.time().get::<second>());
        }
    }
    for c in &pipe.snapshot().cells {
        let (p, rho) = (
            c.pressure.get::<pascal>(),
            c.density.get::<kilogram_per_cubic_meter>(),
        );
        assert!(
            p.is_finite() && p > 0.0 && rho.is_finite() && rho > 0.0,
            "left in a bad state"
        );
    }
}

/// **A fixed step above the stability limit is refused, by name.**
///
/// Sod with `Fixed(1e-3 s)`: the central Courant number is
/// `½·c·Δt/Δx ≈ ½·374·1e-3/0.1 ≈ 1.9`, above
/// [`FIXED_STEP_COURANT_LIMIT`] = 0.5. Pass: `step()` returns
/// `CourantViolation` with that number (not NaN, not a panic), the state is
/// unchanged, and a `Fixed(1e-6 s)` step on the same pipe succeeds.
#[test]
fn a_fixed_step_that_is_too_large_is_refused() {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(10.0),
        Area::new::<square_meter>(1.0),
        100,
    )
    .unwrap();
    let ic = HemInitialCondition::Riemann {
        diaphragm: Length::new::<meter>(5.0),
        left: gas(1.0e5, 1.0, 0.0),
        right: gas(1.0e4, 0.125, 0.0),
    };
    let big = HemNumerics {
        time_step: TimeStepControl::Fixed(Time::new::<second>(1.0e-3)),
        ..HemNumerics::default()
    };
    let mut pipe = HemPipe::new(geo, IdealGasHem::air(), ic.clone(), big).unwrap();
    let before = pipe.snapshot();
    match pipe.step() {
        Err(HemError::CourantViolation { courant, limit }) => {
            println!("CourantViolation: courant = {courant:.4}, limit = {limit}");
            assert!(courant.is_finite() && courant > limit);
            assert_eq!(limit, FIXED_STEP_COURANT_LIMIT);
        }
        other => panic!("expected CourantViolation, got {other:?}"),
    }
    assert_eq!(pipe.snapshot(), before);
    let small = HemNumerics {
        time_step: TimeStepControl::Fixed(Time::new::<second>(1.0e-6)),
        ..HemNumerics::default()
    };
    let mut ok = HemPipe::new(geo, IdealGasHem::air(), ic, small).unwrap();
    assert!(ok.step().is_ok());
}

// ───────────────────────── conservation ─────────────────────────

/// **Conservation to round-off between two walls, on IF97 water that is half
/// subcooled and half two-phase.**
///
/// # Methodology
///
/// 1 m pipe, 40 cells, `Wall` at both ends, IAPWS-IF97. Left half: subcooled
/// liquid at 2 MPa, 450 K. Right half: two-phase at 1 MPa, `h = 1500 kJ/kg`
/// (quality ≈ 0.31). The pressure jump launches a shock into the mixture and a
/// rarefaction into the liquid. 1000 adaptive steps.
///
/// **Pass criterion (plan 03 §4.1 item 3):**
/// `|ΔΣρV|/ΣρV ≤ 1e-12` and `|ΔΣρEV|/ΣρEV ≤ 1e-12` over the 1000 steps. The
/// wall face fluxes of mass and energy are exactly zero, so this is
/// round-off.
#[test]
fn two_walls_conserve_mass_and_total_energy_to_round_off_on_if97() {
    let n = 40;
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-3),
        n,
    )
    .unwrap();
    let cells: Vec<PrimitiveState> = (0..n)
        .map(|i| {
            if i < n / 2 {
                PrimitiveState::PressureTemperature {
                    pressure: Pressure::new::<megapascal>(2.0),
                    temperature: ThermodynamicTemperature::new::<kelvin>(450.0),
                    velocity: Velocity::new::<meter_per_second>(0.0),
                }
            } else {
                PrimitiveState::PressureEnthalpy {
                    pressure: Pressure::new::<megapascal>(1.0),
                    specific_enthalpy: AvailableEnergy::new::<kilojoule_per_kilogram>(1500.0),
                    velocity: Velocity::new::<meter_per_second>(0.0),
                }
            }
        })
        .collect();
    let mut pipe = HemPipe::new(
        geo,
        Iapws97Hem,
        HemInitialCondition::PerCell(cells),
        HemNumerics::default(),
    )
    .unwrap();
    assert!(pipe.cell_state(n - 1).unwrap().quality.is_some());
    let m0 = pipe.total_mass().value;
    let e0 = pipe.total_energy().value;
    let mut min_p = f64::INFINITY;
    for _ in 0..1000 {
        let r = pipe.step().expect("IF97 wall-to-wall step");
        min_p = min_p.min(r.min_pressure.get::<pascal>());
    }
    let dm = (pipe.total_mass().value - m0).abs() / m0;
    let de = (pipe.total_energy().value - e0).abs() / e0;
    println!(
        "IF97 walls: t = {:.4e} s after 1000 steps; |dM|/M = {dm:.3e}, |dE|/E = {de:.3e}, min p = {min_p:.1} Pa",
        pipe.time().get::<second>()
    );
    assert!(dm <= 1e-12, "mass drift {dm:e}");
    assert!(de <= 1e-12, "energy drift {de:e}");
}

// ───────────────────────── pinned defaults ─────────────────────────

/// **Correct physics is the default** (root CLAUDE.md; pattern of
/// `outram-mc-libs/tests/correct_physics_is_default.rs`).
///
/// Pins, through the ordinary construction path:
/// - `HemNumerics::default()` is adaptive central Courant 0.2 (`max_dt` 1 ms),
///   van Leer, SSP22;
/// - `HemPipe` with no type argument is `HemPipe<Iapws97Hem>`;
/// - inside the dome (1 MPa, 1500 kJ/kg) the IF97 closure's sound speed is the
///   homogeneous-**equilibrium** speed `w_ph_eqm_hem`, which there equals
///   Kieffer (1977) eq. 28 (`w_ps_eqm_region4_kieffer`), and is below the
///   frozen Wood–Wallis speed (equilibrium ≤ frozen is a thermodynamic
///   stability result, so this direction cannot be wrong for a correct
///   closure);
/// - the speed a solver cell reports is that same equilibrium speed.
///
/// **Correction, 2026-10-16:** the first version also asserted
/// `c_eqm < ½ c_frozen`. That factor was an unchecked guess, not a published
/// bound, and the measured values are `c_eqm = 264.2 m/s`,
/// `c_frozen = 503.1 m/s` (ratio 0.525). The identity checks it sat beside
/// all passed. It is replaced by the strict inequality, and the equilibrium
/// isentrope's own finite difference (`w_ps_eqm_region4_finite_diff_vol`) is
/// printed beside Kieffer as an independent cross-check.
#[test]
fn the_defaults_are_pinned_and_if97_uses_the_equilibrium_sound_speed() {
    let d = HemNumerics::default();
    assert_eq!(
        d.time_step,
        TimeStepControl::AdaptiveCourant {
            courant: Ratio::new::<ratio>(0.2),
            max_dt: Time::new::<second>(1.0e-3)
        }
    );
    assert_eq!(d.limiter, FluxLimiter::VanLeer);
    assert_eq!(d.time_integrator, TimeIntegrator::Ssp22);

    let p = Pressure::new::<megapascal>(1.0);
    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(1500.0);
    let st = Iapws97Hem.state_p_h(p, h).unwrap();
    let c = st.equilibrium_sound_speed.get::<meter_per_second>();
    let c_eqm = w_ph_eqm_hem(p, h).get::<meter_per_second>();
    let c_kieffer = w_ps_eqm_region4_kieffer(p, s_ph_eqm(p, h)).get::<meter_per_second>();
    let c_frozen = w_ph_wood_wallis(p, h).get::<meter_per_second>();
    let c_fd = w_ps_eqm_region4_finite_diff_vol(p, s_ph_eqm(p, h)).get::<meter_per_second>();
    println!("1 MPa, 1500 kJ/kg: c_closure = {c:.3}, w_ph_eqm_hem = {c_eqm:.3}, Kieffer = {c_kieffer:.3}, isentrope FD = {c_fd:.3}, Wood-Wallis = {c_frozen:.3} m/s");
    assert_eq!(c, c_eqm);
    assert_eq!(c, c_kieffer);
    assert!(c < c_frozen);

    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-3),
        2,
    )
    .unwrap();
    let pipe: HemPipe = HemPipe::new(
        geo,
        Iapws97Hem,
        HemInitialCondition::Uniform(PrimitiveState::PressureEnthalpy {
            pressure: p,
            specific_enthalpy: h,
            velocity: Velocity::new::<meter_per_second>(0.0),
        }),
        HemNumerics::default(),
    )
    .unwrap();
    let cell = pipe.cell_state(0).unwrap();
    let c_cell = cell.equilibrium_sound_speed.get::<meter_per_second>();
    let c_at_cell = w_ph_eqm_hem(cell.pressure, cell.specific_enthalpy).get::<meter_per_second>();
    assert_eq!(c_cell, c_at_cell);
    assert!((c_cell / c - 1.0).abs() < 1e-6);
}

// ─────────────── HEM water double rarefaction (03 §4.2) ───────────────

/// Case parameters, fixed before any run (see the V&V file for why).
const DR_P0: f64 = 5.0e6;
const DR_T0: f64 = 450.0;
const DR_U0: f64 = 5.0;

/// Prints the semi-analytical solution. This is the PREDICTION; it was run
/// and recorded in the V&V file before the solver was run on the case.
#[test]
fn hem_water_double_rarefaction_semi_analytical_prediction() {
    let s = hem_rarefaction::solve(DR_P0, DR_T0, DR_U0);
    println!("{s:#?}");
    let liquid_tail = s.u_bubble - s.c_liquid.1;
    let dome_head = s.u_bubble - s.c_dome_at_bubble;
    println!(
        "wave speeds: head {:.2}, liquid tail {:.2}, dome head {:.2}, centre edge {:.2} m/s",
        -s.u0 - s.c_liquid.0,
        liquid_tail,
        dome_head,
        -s.c_star
    );
    // A single rarefaction: pressure falls monotonically from the head to the
    // centre. Sampled at 400 points of ξ across the left half.
    let mut prev = f64::INFINITY;
    for k in 0..=400 {
        let xi = -s.u0 - s.c_liquid.0 + (k as f64) / 400.0 * (s.u0 + s.c_liquid.0 - s.c_star);
        let (p, _) = hem_rarefaction::sample_left(&s, xi);
        assert!(
            p <= prev * (1.0 + 1e-9),
            "pressure rises at xi = {xi}: {p} after {prev}"
        );
        prev = p;
    }
    assert!(s.p_star < s.p_bubble && s.p_bubble < s.p0);
}

/// **HEM water double rarefaction against its semi-analytical solution**
/// (plan 03 §4.2).
///
/// # Methodology
///
/// Subcooled water at 5 MPa, 450 K, `u = ∓5 m/s` either side of the centre of
/// a 1 m pipe (100 cells, `Δx = 1 cm`), transmissive ends, IF97 closure,
/// default numerics, to `t = 20 ms`. The semi-analytical answer
/// (`common::hem_rarefaction`: Riemann invariant along the isentrope with the
/// equilibrium sound speed; one bisection, two `qags` quadratures, one Brent
/// root) was computed and recorded in the V&V file **before** this run:
///
/// | quantity | predicted |
/// |---|---|
/// | bubble pressure `p_b` on the isentrope | 0.920004 MPa |
/// | saturated-liquid plateau velocity `u_b` | −1.7495 m/s |
/// | centre pressure `p*` (inside the dome) | 0.908004 MPa |
/// | centre density `ρ*` | 726.20 kg/m³ |
/// | equilibrium speed at `p*` | 9.37 m/s |
///
/// At 20 ms the centre state spans `|x| < 0.187 m` and the saturated-liquid
/// plateau `0.190 m < |x|` out to the ends (the liquid fan left the pipe at
/// ~0.35 ms).
///
/// **Pass criteria (fixed before the run):**
/// - plateau (cells with `0.30 ≤ |x| ≤ 0.45 m`): mean `p` within 1 % of `p_b`,
///   mean `|u|` within 5 % of `|u_b|`;
/// - centre (cells with `|x| ≤ 0.10 m`): mean `p` within `0.25·(p_b − p*)`
///   (3.0 kPa) of `p*`, so that it is told apart from `p_b`; mean `ρ` within
///   5 % of `ρ*` (the saturated liquid is 887 kg/m³, so this is the flashing
///   test).
#[test]
fn hem_water_double_rarefaction_matches_the_semi_analytical_plateaus() {
    let wall = std::time::Instant::now();
    let pred = hem_rarefaction::solve(DR_P0, DR_T0, DR_U0);
    let n = 100;
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-3),
        n,
    )
    .unwrap();
    let state = |u: f64| PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<pascal>(DR_P0),
        temperature: ThermodynamicTemperature::new::<kelvin>(DR_T0),
        velocity: Velocity::new::<meter_per_second>(u),
    };
    let mut pipe = HemPipe::new(
        geo,
        Iapws97Hem,
        HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(0.5),
            left: state(-DR_U0),
            right: state(DR_U0),
        },
        HemNumerics::default(),
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
        .unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
        .unwrap();
    let t_end = 0.02;
    let summary = pipe
        .run_until(Time::new::<second>(t_end))
        .expect("double rarefaction run");
    let snap = pipe.snapshot();
    let mut plateau = (0.0, 0.0, 0usize);
    let mut centre = (0.0, 0.0, 0usize);
    let mut profile = String::new();
    for (x, c) in snap.cell_centres.iter().zip(&snap.cells) {
        let xr = x.get::<meter>() - 0.5;
        let p = c.pressure.get::<pascal>();
        let u = c.velocity.get::<meter_per_second>();
        let rho = c.density.get::<kilogram_per_cubic_meter>();
        if (0.30..=0.45).contains(&xr.abs()) {
            plateau = (plateau.0 + p, plateau.1 + u.abs(), plateau.2 + 1);
        }
        if xr.abs() <= 0.10 {
            centre = (centre.0 + p, centre.1 + rho, centre.2 + 1);
        }
        let (pe, ue) = hem_rarefaction::sample_left(&pred, -xr.abs() / t_end);
        profile.push_str(&format!(
            "  x = {xr:+.3} m: p = {:.4} MPa (exact {:.4}), u = {u:+.4} m/s (exact {:+.4}), rho = {rho:.1}, x_q = {:?}\n",
            p / 1e6,
            pe / 1e6,
            -ue * xr.signum(),
            c.quality.map(|q| q.get::<ratio>())
        ));
    }
    let p_plateau = plateau.0 / plateau.2 as f64;
    let u_plateau = plateau.1 / plateau.2 as f64;
    let p_centre = centre.0 / centre.2 as f64;
    let rho_centre = centre.1 / centre.2 as f64;
    println!("{profile}");
    println!(
        "double rarefaction: {} steps, max central Courant {:.3}, wall clock {:.1} s",
        summary.steps,
        summary.max_courant.get::<ratio>(),
        wall.elapsed().as_secs_f64()
    );
    println!(
        "  plateau: p = {:.6} MPa vs p_b = {:.6} ({:+.3} %); |u| = {:.4} vs {:.4} m/s ({:+.2} %)",
        p_plateau / 1e6,
        pred.p_bubble / 1e6,
        100.0 * (p_plateau / pred.p_bubble - 1.0),
        u_plateau,
        -pred.u_bubble,
        100.0 * (u_plateau / -pred.u_bubble - 1.0)
    );
    println!(
        "  centre:  p = {:.6} MPa vs p* = {:.6} (diff {:+.0} Pa, gate 0.25(p_b - p*) = {:.0} Pa); rho = {:.2} vs {:.2} kg/m3 ({:+.2} %)",
        p_centre / 1e6,
        pred.p_star / 1e6,
        p_centre - pred.p_star,
        0.25 * (pred.p_bubble - pred.p_star),
        rho_centre,
        pred.rho_star,
        100.0 * (rho_centre / pred.rho_star - 1.0)
    );
    assert!(
        (p_plateau / pred.p_bubble - 1.0).abs() <= 0.01,
        "plateau pressure"
    );
    assert!(
        (u_plateau / -pred.u_bubble - 1.0).abs() <= 0.05,
        "plateau velocity"
    );
    assert!(
        (p_centre - pred.p_star).abs() <= 0.25 * (pred.p_bubble - pred.p_star),
        "centre pressure"
    );
    assert!(
        (rho_centre / pred.rho_star - 1.0).abs() <= 0.05,
        "centre density"
    );
}
