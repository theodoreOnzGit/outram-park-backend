//! Tests of [`super::model::Ebr2Model`]. Each V&V test states its
//! prediction (written before the first run) and its result. The results
//! are collected in `crates/outram-park-digital-twin-engine/docs/ebr2_library.md`.

use uom::si::angular_velocity::revolution_per_minute;
use uom::si::electric_current::ampere;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::{kilopascal, pound_force_per_square_inch};
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::kinetics::{ArgonneSolver, PCM};
use super::model::*;
use super::shrt::{self, ShrtTest};

fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn s(x: f64) -> Time {
    Time::new::<second>(x)
}

/// Summary of one transient run.
#[derive(Debug, Clone, Copy)]
struct RunSummary {
    peak_pf: f64,
    peak_pf_time: f64,
    max_outlet_k: f64,
    max_outlet_time: f64,
    max_fuel_k: f64,
    min_flow_fraction: f64,
    power_at_60s_mw: f64,
    power_at_300s_mw: f64,
    power_at_900s_mw: f64,
    flow_at_300s: f64,
    flow_at_620s: f64,
    flow_at_900s: f64,
    min_reactivity_pcm: f64,
    max_reactivity_pcm: f64,
    max_reactivity_time: f64,
    min_fission_fraction: f64,
}

/// Run to 900 s in 0.5 s steps, checking finiteness at every step.
fn run(mut model: Ebr2Model) -> (Ebr2Model, RunSummary) {
    let w0 = model.primary_flow().get::<kilogram_per_second>();
    let mut sum = RunSummary {
        peak_pf: 0.0,
        peak_pf_time: 0.0,
        max_outlet_k: 0.0,
        max_outlet_time: 0.0,
        max_fuel_k: 0.0,
        min_flow_fraction: f64::INFINITY,
        power_at_60s_mw: 0.0,
        power_at_300s_mw: 0.0,
        power_at_900s_mw: 0.0,
        flow_at_300s: 0.0,
        flow_at_620s: 0.0,
        flow_at_900s: 0.0,
        min_reactivity_pcm: f64::INFINITY,
        max_reactivity_pcm: f64::NEG_INFINITY,
        max_reactivity_time: 0.0,
        min_fission_fraction: f64::INFINITY,
    };
    while model.time().get::<second>() < 900.0 - 1e-9 {
        model.step(s(0.5)).expect("step");
        let t = model.time().get::<second>();
        let pf = model.power_to_flow_ratio().get::<ratio>();
        let out = model.core_outlet_temperature().get::<kelvin>();
        let fuel = model.fuel_temperature().get::<kelvin>();
        let w = model.primary_flow().get::<kilogram_per_second>() / w0;
        let p = model.total_power().get::<megawatt>();
        let rho = model.reactivity().total().get::<ratio>() / PCM;
        for v in [pf, out, fuel, w, p, rho] {
            assert!(v.is_finite(), "non-finite state at {t} s");
        }
        assert!(p >= 0.0);
        if pf > sum.peak_pf {
            sum.peak_pf = pf;
            sum.peak_pf_time = t;
        }
        if out > sum.max_outlet_k {
            sum.max_outlet_k = out;
            sum.max_outlet_time = t;
        }
        sum.max_fuel_k = sum.max_fuel_k.max(fuel);
        sum.min_flow_fraction = sum.min_flow_fraction.min(w);
        sum.min_fission_fraction = sum
            .min_fission_fraction
            .min(model.fission_power().get::<megawatt>() / p);
        sum.min_reactivity_pcm = sum.min_reactivity_pcm.min(rho);
        if rho > sum.max_reactivity_pcm {
            sum.max_reactivity_pcm = rho;
            sum.max_reactivity_time = t;
        }
        let at = |x: f64| (t - x).abs() < 0.25;
        if at(60.0) {
            sum.power_at_60s_mw = p;
        }
        if at(300.0) {
            sum.power_at_300s_mw = p;
            sum.flow_at_300s = w;
        }
        if at(620.0) {
            sum.flow_at_620s = w;
        }
        if at(900.0) {
            sum.power_at_900s_mw = p;
            sum.flow_at_900s = w;
        }
    }
    (model, sum)
}

/// **The model's own steady state against the cited initial conditions
/// (uncalibrated).**
///
/// Methodology: build each test's model at the default geometry with
/// `dz = 2 m`; the IHX thermal-centre height barely matters at full flow.
/// Compare four things:
/// - the flow from the NATDEMO curve at the tabulated t = 0 pump speeds
///   against A226's initial flow (SHRT-17 through the core; SHRT-45R at the
///   pumps);
/// - the pool (core inlet) temperature, which follows from the IHX UA and
///   the intermediate boundary conditions, against A226's initial core
///   inlet;
/// - the fuel minus mean-coolant temperature against A228 Table 3.3's
///   Run 138B averages (487 - 393 = 94 K at 60 MW);
/// - the mean pump head against A226's discharge pressures.
///
/// Predictions, written before the first run (2026-11-10):
/// - SHRT-17 flow within +-5 % of 455 kg/s (hand estimate +1 %);
/// - SHRT-45R flow about -7 % against 8971.9 gpm at the pumps (its pump
///   speeds are lower, at the same rated loop);
/// - core inlet within +-30 K of the cited value;
/// - fuel - coolant 55-85 K against 94 K (under-predicted: fully dense fuel
///   conducts better than burned fuel);
/// - pump head about 300 kPa against 296 kPa (42.9 psi).
///
/// The assertions are sanity bounds only. The comparison is the printed
/// numbers, recorded in `docs/ebr2_library.md`.
///
/// Results (2026-11-10, deterministic, no statistical uncertainty):
///
/// | | SHRT-17 | SHRT-45R |
/// |---|---|---|
/// | flow, model vs cited (kg/s) | 449.7 vs 454.1 (-1.0 %) | 447.6 vs 479.3 at the pumps (-6.6 %) |
/// | core inlet, model vs cited (K) | 618.13 vs 624.71 (-6.58 K) | 621.36 vs 616.93 (+4.43 K) |
/// | core outlet (K) | 717.35 | 725.87 |
/// | fuel - mean coolant (K) | 68.3 | 71.2 vs 94 (Run 138B, -24 %) |
/// | pump head (kPa) | 289.0 vs 292.3 (mean of 42.9 and 41.9 psi) | 285.2 vs 288.5 (41.84 psi) |
/// | IHX UA (W/K), effectiveness | 4.024e6, 0.983 | 4.012e6, 0.993 |
///
/// Interpretation: flow, inlet temperature and pump head agree with the
/// cited values to a few percent and a few kelvin, uncalibrated. The fuel
/// temperature rise is under-predicted by 24 %, in the predicted direction:
/// the model's fuel is fully dense, with no porosity correction.
#[test]
fn steady_state_against_cited_initial_conditions() {
    for test in [ShrtTest::Shrt17, ShrtTest::Shrt45r] {
        let model = match test {
            ShrtTest::Shrt17 => Ebr2Model::shrt17(m(2.0)).unwrap(),
            ShrtTest::Shrt45r => Ebr2Model::shrt45r(m(2.0)).unwrap(),
        };
        let c = model.initial_comparison();
        let w = c.primary_flow.get::<kilogram_per_second>();
        let w_cited = c.cited_primary_flow.get::<kilogram_per_second>();
        let t_in = c.core_inlet.get::<kelvin>();
        let t_in_cited = c.cited_core_inlet.get::<kelvin>();
        let d_fc = c.fuel_minus_coolant.get::<kelvin_interval>();
        let head = c.pump_head.get::<kilopascal>();
        let br = model.conductance_breakdown().unwrap();
        let (ihx_ua, eps) = model.ihx_state().unwrap();
        let (core_na, ihx_na, pool_na) = model.sodium_masses();
        println!(
            "{}: P0 {:.2} MW | flow {w:.1} vs cited {w_cited:.1} kg/s ({:+.1} %) | inlet {t_in:.2} vs cited {t_in_cited:.2} K ({:+.2} K) | outlet {:.2} K | fuel-coolant {d_fc:.1} K (cited Run 138B 94 K) | pump head {head:.1} kPa | UA_fc {:.4e} W/K (fuel {:.3e} bond {:.3e} clad {:.3e} film {:.3e} m K/W, Pe {:.1}, Nu {:.2}) | IHX UA {:.4e} W/K eps {eps:.4} | Na masses core {:.0} IHX {:.0} pool {:.0} kg",
            test.name(),
            model.total_power().get::<megawatt>(),
            100.0 * (w / w_cited - 1.0),
            t_in - t_in_cited,
            c.core_outlet.get::<kelvin>(),
            br.ua,
            br.fuel,
            br.bond,
            br.cladding,
            br.film,
            br.peclet,
            br.nusselt,
            ihx_ua.value,
            core_na.value,
            ihx_na.value,
            pool_na.value,
        );
        assert!(w > 300.0 && w < 600.0);
        assert!(t_in > 550.0 && t_in < 750.0);
        assert!(d_fc > 0.0 && d_fc < 300.0);
        assert!(c.core_outlet > c.core_inlet);
        assert!(eps > 0.0 && eps <= 1.0);
        let psi = Pressure::new::<kilopascal>(head).get::<pound_force_per_square_inch>();
        assert!(psi > 30.0 && psi < 55.0, "{psi}");
    }
}

/// **SHRT-17 over its tabulated 900 s: finite, below boiling, power on the
/// table.**
///
/// Methodology: tabulated power, the A226 pump-speed and intermediate-side
/// tables, 0.5 s steps (0.05 s sub-steps). The thermal-centre height `dz`
/// is not in the sources, so it is swept over 1, 2 and 4 m. At every step
/// the state must be finite and every sodium node inside TUAS's 400-1000 K
/// (below boiling; `step` errors otherwise). The power must equal the table
/// at 900 s.
///
/// The one measured quantity available as a number is in T1819 §3.3 as
/// transcribed in gh:#1316: the normalised power-to-flow ratio peaked at
/// **2.4 at about 1 min** (read from the text, not a trace; unreviewed).
///
/// Predictions, written before the first run (2026-11-10):
/// - bounded for every `dz`;
/// - peak P/F between 1 and 3, between 35 and 90 s, rising as `dz` falls;
/// - for `dz` = 2 m, peak core outlet 700-900 K.
///
/// Results (2026-11-10): every prediction held.
///
/// | dz | peak P/F (time) | peak core outlet (time) | flow at 300 s |
/// |---|---|---|---|
/// | 1 m | 2.47 (50.0 s) | 797.0 K (65.5 s) | 1.51 % |
/// | 2 m | 1.65 (49.5 s) | 753.7 K (59.0 s) | 2.05 % |
/// | 4 m | 1.16 (48.5 s) | 721.6 K (55.5 s) | 2.76 % |
///
/// Interpretation: the peak P/F depends strongly on `dz`, which no
/// transcribed source gives. The measured 2.4 does not identify `dz`, and
/// must not be used to choose it: that would make the check an input. The
/// model's peak comes about 10 s earlier than "about 1 min". With no fluid
/// inertia in the loop, the flow falls with the pump speed.
#[test]
fn shrt17_run_stays_bounded_and_follows_the_table() {
    for dz in [1.0, 2.0, 4.0] {
        let model = Ebr2Model::shrt17(m(dz)).unwrap();
        let p0 = model.total_power().get::<megawatt>();
        assert!((p0 - 57.29).abs() < 1e-9, "{p0}");
        let (model, sum) = run(model);
        println!("SHRT-17 dz = {dz} m: {sum:?}");
        // the power reported is the one applied over the last sub-step:
        // the table at that sub-step's midpoint, 900 - 0.025 s
        let (f, d) = ShrtTest::Shrt17.tabulated_power(s(900.0 - 0.025)).unwrap();
        assert!((model.total_power() - (f + d)).get::<megawatt>().abs() < 1e-12);
        assert!(sum.max_outlet_k < 1000.0);
        assert!(sum.peak_pf > 0.5 && sum.peak_pf < 10.0);
        assert!(model.reactivity().total().get::<ratio>() == 0.0);
        assert!(model.beta_eff().is_none());
        let pool = model.pool_temperature().get::<kelvin>();
        assert!(pool > 550.0 && pool < 700.0);
    }
}

/// **SHRT-45R with Argonne's VARIANT coefficients: bounded, self-limiting.**
///
/// Methodology: point kinetics (beta 705 pcm, Lambda 3.58e-7 s), the
/// feedback set at the initial steady state, decay heat from A226's assumed
/// pre-test history, `dz = 2 m`, 0.5 s steps. No scram.
///
/// Ablations, each changing one choice: radial expansion on the mean
/// coolant or on the outlet temperature; the auxiliary EM pump off;
/// DIF3D's coefficients; `dz` of 1 m and 4 m.
///
/// Measured behaviour available as numbers (T1819 §3.3 via gh:#1316,
/// unreviewed): P/F peaked at **2.8 at about 1 min**; fission power was more
/// than half the total power throughout; natural-circulation flow was more
/// than double SHRT-17's; flow rose about 20 % after the auxiliary pump's
/// current rose 50 % at 600 s.
///
/// Predictions, written before the first run (2026-11-10):
/// - bounded, with net reactivity negative after the first seconds and
///   never above +0.1 beta;
/// - power falling to 5-25 % of the initial 60 MW by 300 s;
/// - peak P/F between 1.5 and 4;
/// - natural-circulation flow at 300 s above SHRT-17's at the same `dz`;
/// - the flow ratio from 300 s to 620 s rising by under 20 %, because the
///   +50 % current also has to overcome the natural-circulation share.
///
/// Results (2026-11-10), base case (VARIANT, radial on inlet, dz 2 m):
/// - peak P/F **3.18 at 74.0 s** against the measured 2.8 at about 1 min;
/// - peak core outlet 956.1 K at 76.5 s;
/// - power 18.96 MW at 60 s and 5.76 MW (9.6 %) at 300 s;
/// - net reactivity from -217.0 pcm to **+15.3 pcm at 655.5 s**;
/// - smallest fission share of total power 0.745;
/// - flow at 300 s 6.05 % of initial, against SHRT-17's 2.05 % (2.95x);
/// - flow from 620 s to 900 s, across the current step, +19.5 %.
///
/// Every prediction held but one. Net reactivity turns **positive** (at
/// most +0.022 beta) after the auxiliary pump's current step at about
/// 630 s, which cools the core below the temperature of the feedback
/// references. The ablations (peak P/F, peak outlet K, power at 300 s in
/// MW):
/// - radial expansion on mean coolant: 2.56, 890.6, 4.68;
/// - radial expansion on outlet: 2.26, 857.7, 4.24;
/// - DIF3D coefficients: 3.15, 953.2, 5.34;
/// - aux EM pump off: 3.52, 990.5, 3.99;
/// - dz 1 m: 3.38, 975.1, 5.13;
/// - dz 4 m: 2.98, 935.8, 6.62.
///
/// The temperature that drives radial expansion is the largest uncited
/// modelling choice: it moves the peak outlet by 98 K.
///
/// **Re-measured 2026-11-10 (gh:#1319): the six-group shape changed.**
/// `teh-o-prke`'s U-235 default moved from its textbook set to
/// ENDF/B-VIII.0 (mean precursor life 13.05 s to 11.06 s; beta still
/// rescaled to 705 pcm). Same command, same inputs otherwise. Predicted on
/// gh:#1319 before the run: fission-power changes of at most a few %, peak
/// temperatures within +-2 K. **The temperature prediction missed**: the
/// shorter-lived groups let the fission power fall faster after the flow
/// coastdown, so the peak outlet fell 19.3 K. The peak P/F moved toward the
/// measured 2.8; that is a consequence of the data choice, not a fit.
/// Base case, before -> after:
/// - peak P/F ~~3.18 at 74.0 s~~ **3.00 at 74.5 s** (measured 2.8 at about 1 min);
/// - peak core outlet ~~956.1 K at 76.5 s~~ **936.8 K at 77.0 s**;
/// - power ~~18.96~~ **17.79** MW at 60 s and ~~5.76~~ **5.50** MW (9.2 %) at 300 s;
/// - net reactivity from ~~-217.0~~ **-195.5** pcm to ~~+15.3 pcm at 655.5 s~~
///   **+15.5 pcm at 655.0 s**;
/// - smallest fission share of total power ~~0.745~~ **0.741**;
/// - flow at 300 s ~~6.05 %~~ **5.99 %** of initial (~~2.95x~~ **2.92x** SHRT-17's 2.05 %);
/// - flow from 620 s to 900 s ~~+19.5 %~~ **+19.7 %**.
///
/// Ablations after the change (peak P/F, peak outlet K, power at 300 s in
/// MW; before in the list above):
/// - radial expansion on mean coolant: 2.42, 875.2, 4.54;
/// - radial expansion on outlet: 2.14, 844.6, 4.14;
/// - DIF3D coefficients: 2.96, 933.2, 5.06;
/// - aux EM pump off: 3.35, 970.9, 3.72;
/// - dz 1 m: 3.17, 953.4, 4.90;
/// - dz 4 m: 2.82, 919.0, 6.35.
///
/// The radial-expansion temperature choice still dominates (92 K between
/// inlet and outlet), larger than the delayed-data change (19 K).
#[test]
fn shrt45r_run_stays_bounded_and_self_limits() {
    let start = std::time::Instant::now();
    let (_, base) = run(Ebr2Model::shrt45r(m(2.0)).unwrap());
    let wall = start.elapsed().as_secs_f64();
    println!(
        "SHRT-45R VARIANT, radial on inlet, dz 2 m: {base:?}; wall {wall:.3} s for 18000 sub-steps ({:.1} us each)",
        wall / 18000.0 * 1e6
    );
    assert!(base.max_outlet_k < 1000.0);
    assert!(base.max_reactivity_pcm < 0.1 * 705.0);
    assert!(base.power_at_300s_mw < 60.0);

    let (_, s17) = run(Ebr2Model::shrt17(m(2.0)).unwrap());
    println!(
        "natural-circulation flow fraction at 300 s: SHRT-45R {:.4}, SHRT-17 {:.4}",
        base.flow_at_300s, s17.flow_at_300s
    );

    let variants: [(&str, Ebr2Config); 6] = {
        let mut mean = Ebr2Config::shrt45r(m(2.0));
        let mut outlet = Ebr2Config::shrt45r(m(2.0));
        let mut dif3d = Ebr2Config::shrt45r(m(2.0));
        if let PowerConfig::PointKinetics(k) = &mut mean.power {
            k.radial_expansion_temperature = RadialExpansionTemperature::CoreMeanCoolant;
        }
        if let PowerConfig::PointKinetics(k) = &mut outlet.power {
            k.radial_expansion_temperature = RadialExpansionTemperature::CoreOutlet;
        }
        if let PowerConfig::PointKinetics(k) = &mut dif3d.power {
            k.coefficients = ReactivityCoefficients::argonne(ArgonneSolver::Dif3dFdd);
        }
        let mut no_aux = Ebr2Config::shrt45r(m(2.0));
        no_aux.aux_em_pump = false;
        [
            ("radial on mean coolant", mean),
            ("radial on outlet", outlet),
            ("DIF3D coefficients", dif3d),
            ("aux EM pump off", no_aux),
            ("dz 1 m", Ebr2Config::shrt45r(m(1.0))),
            ("dz 4 m", Ebr2Config::shrt45r(m(4.0))),
        ]
    };
    for (name, config) in variants {
        let (_, sum) = run(Ebr2Model::new(config).unwrap());
        println!("SHRT-45R ablation, {name}: {sum:?}");
        assert!(sum.max_outlet_k < 1000.0, "{name}");
    }
}

/// The initial state is stationary when the inputs are held at their t = 0
/// values (kinetics mode). The only drift is the decay heat, whose long
/// groups are not saturated after A226's 5.67-day history.
///
/// Prediction: under 0.1 K and under 0.5 pcm over 20 s.
///
/// Result (2026-11-10): at most 8.3e-6 K and 0.0000 pcm. The first run
/// drifted 0.2 K and exposed two inconsistencies:
/// - the heat capacity was evaluated at different temperatures in the
///   initialiser and the step;
/// - the history's total powers were fed to the decay model as fission
///   powers.
///
/// Both are fixed (`loop_heat_capacity`, `history_decay_heat`).
#[test]
fn initial_state_is_stationary_with_inputs_held() {
    let mut model = Ebr2Model::shrt45r(m(2.0)).unwrap();
    let i = model.inputs();
    model.set_pump_speeds(Some(i.pump_speeds));
    model.set_intermediate_flow(Some(i.intermediate_flow));
    model.set_intermediate_inlet_temperature(Some(i.intermediate_inlet_temperature));
    model.set_aux_em_pump_current(Some(i.aux_em_pump_current));
    let t0 = [
        model.fuel_temperature(),
        model.core_mean_coolant_temperature(),
        model.ihx_primary_outlet_temperature(),
        model.pool_temperature(),
    ];
    model.step(s(20.0)).unwrap();
    let t1 = [
        model.fuel_temperature(),
        model.core_mean_coolant_temperature(),
        model.ihx_primary_outlet_temperature(),
        model.pool_temperature(),
    ];
    for (a, b) in t0.iter().zip(t1) {
        let d = (b.get::<kelvin>() - a.get::<kelvin>()).abs();
        println!("drift over 20 s: {d:.3e} K");
        assert!(d < 0.1);
    }
    let rho = model.reactivity().total().get::<ratio>() / PCM;
    println!("reactivity after 20 s held: {rho:.4} pcm");
    assert!(rho.abs() < 0.5);
    assert_eq!(model.overrides().pump_speeds, Some(i.pump_speeds));
}

/// Stepping is deterministic, and one long step equals the same sub-steps
/// taken one by one, bit for bit.
#[test]
fn stepping_is_deterministic_and_substep_invariant() {
    let mut a = Ebr2Model::shrt45r(m(2.0)).unwrap();
    let mut b = a.clone();
    a.step(s(0.5)).unwrap();
    for _ in 0..10 {
        b.step(s(0.05)).unwrap();
    }
    assert_eq!(a.total_power(), b.total_power());
    assert_eq!(a.core_outlet_temperature(), b.core_outlet_temperature());
    assert_eq!(a.time(), b.time());
    let mut c = Ebr2Model::shrt45r(m(2.0)).unwrap();
    c.step(s(0.5)).unwrap();
    assert_eq!(a.fuel_temperature(), c.fuel_temperature());
    // dt = 0 is a no-op; a negative or NaN dt is refused
    c.step(s(0.0)).unwrap();
    assert_eq!(c.time(), a.time());
    assert!(c.step(s(-1.0)).is_err());
    assert!(c.step(s(f64::NAN)).is_err());
}

/// The setters reach the physics: rated pump speeds raise the flow; a
/// positive external reactivity raises the power in kinetics mode and is
/// refused in tabulated mode; the overrides show in `inputs()`.
#[test]
fn setters_reach_the_physics() {
    let mut model = Ebr2Model::shrt17(m(2.0)).unwrap();
    model.step(s(30.0)).unwrap();
    let w_table = model.primary_flow();
    let rated = AngularVelocity::new::<revolution_per_minute>(870.0);
    model.set_pump_speeds(Some((rated, rated)));
    model.step(s(1.0)).unwrap();
    assert!(model.primary_flow() > w_table);
    let w_rated = model.primary_flow().get::<kilogram_per_second>();
    println!("flow at rated speed: {w_rated:.2} kg/s");
    assert!((w_rated / 500.4 - 1.0).abs() < 0.05);
    assert!(model.set_external_reactivity(Ratio::new::<ratio>(1e-4)).is_err());
    let hot = ThermodynamicTemperature::new::<kelvin>(600.0);
    let flow = MassRate::new::<kilogram_per_second>(100.0);
    let amps = ElectricCurrent::new::<ampere>(1000.0);
    model.set_intermediate_inlet_temperature(Some(hot));
    model.set_intermediate_flow(Some(flow));
    model.set_aux_em_pump_current(Some(amps));
    model.step(s(0.1)).unwrap();
    let i = model.inputs();
    assert_eq!(i.intermediate_inlet_temperature, hot);
    assert_eq!(i.intermediate_flow, flow);
    assert_eq!(i.aux_em_pump_current, amps);
    model.set_pump_speeds(None);
    model.step(s(0.1)).unwrap();
    assert_eq!(model.inputs().pump_speeds, ShrtTest::Shrt17.pump_speeds(model.time()));

    let mut k = Ebr2Model::shrt45r(m(2.0)).unwrap();
    let p0 = k.total_power();
    k.set_external_reactivity(Ratio::new::<ratio>(10.0 * PCM)).unwrap();
    k.step(s(0.2)).unwrap();
    assert!(k.total_power() > p0);
    assert!((k.reactivity().external.get::<ratio>() - 10.0 * PCM).abs() < 1e-18);
    assert_eq!(k.beta_eff().unwrap().get::<ratio>(), 705.0 * PCM);
    assert_eq!(k.config().test, ShrtTest::Shrt45r);
    assert!(k.decay_power().get::<megawatt>() > 0.0);
    assert!(k.fission_power() > k.decay_power());
}

/// Invalid configurations are refused.
#[test]
fn invalid_configurations_are_refused() {
    let mut c = Ebr2Config::shrt45r(m(2.0));
    c.power = PowerConfig::Tabulated;
    assert!(Ebr2Model::new(c).is_err());
    assert!(Ebr2Model::shrt17(m(-1.0)).is_err());
    assert!(Ebr2Model::shrt17(m(f64::NAN)).is_err());
}

/// **Sensitivity to the IHX side assignment** (not in the transcribed
/// data). Prediction: under 20 % on UA and under 10 K on the steady core
/// inlet.
///
/// Result (2026-11-10): UA 4.0244e6 W/K with primary on the shell side,
/// 4.0255e6 in the tubes (+0.03 %); the core inlet moves under 0.01 K. The
/// assignment does not matter for this model: the two sides' films are
/// similar, and the wall and both films are in series.
#[test]
fn ihx_side_assignment_sensitivity() {
    let shell = Ebr2Model::shrt17(m(2.0)).unwrap();
    let mut config = Ebr2Config::shrt17(m(2.0));
    config.geometry.ihx_primary_side = IhxPrimarySide::Tube;
    let tube = Ebr2Model::new(config).unwrap();
    let (ua_s, _) = shell.ihx_state().unwrap();
    let (ua_t, _) = tube.ihx_state().unwrap();
    let dt = (tube.core_inlet_temperature().get::<kelvin>()
        - shell.core_inlet_temperature().get::<kelvin>())
    .abs();
    println!(
        "IHX UA: primary on shell {:.4e}, in tubes {:.4e} W/K ({:+.1} %); core inlet moves {dt:.2} K",
        ua_s.value,
        ua_t.value,
        100.0 * (ua_t.value / ua_s.value - 1.0)
    );
    assert!(dt < 50.0);
    let d = shell.derived_geometry();
    assert!(d.ihx_pitch_to_diameter > 1.29 && d.ihx_pitch_to_diameter < 1.31);
    assert!(d.bundle_hydraulic_diameter_m > 0.002 && d.bundle_hydraulic_diameter_m < 0.004);
    assert!(d.pins == 87.0 * 91.0);
    assert!(d.fuel_mass_kg > 400.0 && d.fuel_mass_kg < 500.0, "{}", d.fuel_mass_kg);
}

/// The counter-flow effectiveness formula: its limits and a hand value.
#[test]
fn counterflow_effectiveness_limits() {
    assert_eq!(counterflow_effectiveness(1.0, 0.0, 1.0), 0.0);
    assert!((counterflow_effectiveness(1.0, 1.0, 1.0) - 0.5).abs() < 1e-15);
    // NTU = 1, Cr = 0.5: (1 - e^-0.5) / (1 - 0.5 e^-0.5)
    //   = 0.393469 / 0.696735 = 0.564733
    let e = counterflow_effectiveness(1.0, 1.0, 2.0);
    assert!((e - 0.564_733_4).abs() < 1e-6, "{e}");
    assert!(counterflow_effectiveness(1e6, 1.0, 2.0) > 0.999_999);
}

/// The density slope and the wire-wrap P/D are what the module doc says.
#[test]
fn derived_quantities() {
    let slope = sodium_density_slope(ThermodynamicTemperature::new::<kelvin>(666.0)).unwrap();
    println!("sodium d rho/dT at 666 K: {slope:.4} kg/m3/K");
    assert!(slope < -0.15 && slope > -0.35);
    let pd = wire_wrap_pitch_to_diameter().get::<ratio>();
    assert!((pd - (4.420 + 1.245) / 4.420).abs() < 1e-12);
    let rho = shrt::calibration_density_kg_per_m3(shrt::primary_flow_calibration_temperature()).unwrap();
    assert!(rho > 820.0 && rho < 870.0);
    assert!(HIGH_PRESSURE_POSITIONS == 3 * 7 * 6 + 1);
    let cp = sodium_heat_capacity(ThermodynamicTemperature::new::<kelvin>(666.0)).unwrap();
    assert!(cp.value > 1200.0 && cp.value < 1350.0);
}

/// The SHRT-45R initial decay heat from A226's assumed history.
///
/// Prediction: 2.5-3.5 MW (SHRT-17's specification gives 3.36 MW at
/// 53.93 MW fission for a different history). With history totals scaled to
/// fission powers, fission + decay at the end of the history equals 60 MW.
///
/// Result (2026-11-10): 3.491 MW decay heat, 56.509 MW fission (5.8 % of
/// the total, against SHRT-17's 5.9 %).
#[test]
fn shrt45r_initial_decay_heat_from_the_history() {
    let k = KineticsConfig::argonne_shrt45r();
    let d = history_decay_heat(&k.decay_history).total_decay_heat_power();
    let model = Ebr2Model::shrt45r(m(2.0)).unwrap();
    println!(
        "SHRT-45R initial decay heat {:.4} MW, fission {:.4} MW",
        d.get::<megawatt>(),
        model.fission_power().get::<megawatt>()
    );
    assert_eq!(model.decay_power(), d);
    assert!(d.get::<megawatt>() > 1.0 && d.get::<megawatt>() < 5.0);
    let empty = history_decay_heat(&[]).total_decay_heat_power();
    assert_eq!(empty.value, 0.0);
}

/// **Time-step refinement.** SHRT-45R to 200 s with 0.05 s sub-steps (the
/// default) against 0.01 s sub-steps (`step(0.01)` takes one sub-step).
///
/// Prediction (written before the first run): the scheme is first order,
/// so the peak outlet temperature and the power at 100 s differ by under
/// 1 % between the two.
///
/// Result (2026-11-10): peak outlet 956.126 vs 956.099 K (+0.008 % of the
/// 335 K rise); power at 100 s 12.0226 vs 12.0219 MW (+0.006 %). The
/// default sub-step is converged far below every modelling uncertainty
/// recorded in the other tests.
///
/// Re-measured 2026-11-10 after gh:#1319 (ENDF/B-VIII.0 U-235 group
/// shape): peak outlet ~~956.126 vs 956.099~~ **936.838 vs 936.811 K**
/// (+0.008 %); power at 100 s ~~12.0226 vs 12.0219~~ **11.1281 vs 11.1274
/// MW** (+0.006 %). The refinement differences are unchanged.
#[test]
fn timestep_refinement_shrt45r() {
    let peak = |dt: f64| {
        let mut model = Ebr2Model::shrt45r(m(2.0)).unwrap();
        let mut max_out: f64 = 0.0;
        let mut p100 = 0.0;
        let n = (200.0 / dt).round() as usize;
        for i in 0..n {
            model.step(s(dt)).unwrap();
            max_out = max_out.max(model.core_outlet_temperature().get::<kelvin>());
            if ((i + 1) as f64 * dt - 100.0).abs() < 0.5 * dt {
                p100 = model.total_power().get::<megawatt>();
            }
        }
        (max_out, p100)
    };
    let (out_coarse, p_coarse) = peak(0.05);
    let (out_fine, p_fine) = peak(0.01);
    let t_in = 621.36;
    let d_out = (out_coarse - out_fine) / (out_fine - t_in);
    let d_p = p_coarse / p_fine - 1.0;
    println!(
        "refinement: peak outlet {out_coarse:.3} vs {out_fine:.3} K ({:+.3} % of the rise), power at 100 s {p_coarse:.4} vs {p_fine:.4} MW ({:+.3} %)",
        100.0 * d_out,
        100.0 * d_p
    );
    assert!(d_out.abs() < 0.01 && d_p.abs() < 0.01);
}
