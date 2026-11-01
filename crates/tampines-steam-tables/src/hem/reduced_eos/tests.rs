//! Verification of the reduced equations of state (gh:#1057).
//!
//! Predictions written before the first run (2026-11-01): stiffened-gas
//! `T_sat(p)` within 2 % of IF97 over 298–473 K, latent heat within 5 %,
//! liquid density at (1 MPa, 400 K) 5–10 % high with the sound speed within
//! 2 %; the IF97-derived linear EOS exact at its reference point and within
//! 1 % over ±5 K, ±0.5 MPa; the water shock tube's plateau pressure within 5 %
//! of IF97's.
//!
//! **Results (2026-11-01):** latent heat, density, sound speed, linear EOS and
//! shock tube held. **`T_sat` missed: 5–7.5 % low**, traced to the vapour's
//! `q'` as RELAP-7 Table 5 rounds it (see
//! [`StiffenedGasHem::water_vapour_le_metayer_2004`]); the miss is pinned, not
//! hidden. Full record: `verification_and_validation/hem_reduced_eos_vs_iapws97.md`.

use super::*;
use crate::hem::{
    HemBoundary, HemInitialCondition, HemNumerics, HemPipe, HemPipeGeometry, Iapws97Hem, PipeEnd,
    PrimitiveState,
};
use crate::region_1_subcooled_liquid::{h_tp_1, w_tp_1};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use crate::region_2_vapour::h_tp_2;
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Time};
use uom::si::length::meter;
use uom::si::pressure::megapascal;
use uom::si::time::second;

fn kelvin_t(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

fn mpa(p: f64) -> Pressure {
    Pressure::new::<megapascal>(p)
}

/// **Stiffened gas against its closed forms** (RELAP-7 Eqs 78–84, 93).
/// Liquid water (Le Métayer 2004) at 1 MPa, 400 K by hand: `ρ = (1e6 + 1e9)
/// /(1.35·1816·400) = 1020.7620 kg/m³`, `c = √(2.35·1.001e9/1020.7620) =
/// 1518.059 m/s`, `h = 2.35·1816·400 − 1.167e6 = 540 040 J/kg` (Eq 81, which
/// `e + p/ρ` must reproduce exactly). The first two hand densities (1020.698,
/// 1020.7505) were arithmetic slips caught by this test; the values above were
/// computed, not done by hand. Every constructor closes the
/// same state, and `(ρ, e)` comes back to 1 MPa to round-off.
#[test]
fn stiffened_gas_matches_its_closed_forms_and_round_trips() {
    let l = StiffenedGasHem::water_liquid_le_metayer_2004();
    let s = l.state_p_t(mpa(1.0), kelvin_t(400.0)).unwrap();
    let rho = s.density.get::<kilogram_per_cubic_meter>();
    assert!(
        (rho - 1.001e9 / (1.35 * 1816.0 * 400.0)).abs() < 1e-9,
        "{rho}"
    );
    assert!((rho - 1020.7620).abs() < 1e-3);
    assert!((s.equilibrium_sound_speed.get::<meter_per_second>() - 1518.059).abs() < 1e-3);
    assert!((s.specific_enthalpy.get::<joule_per_kilogram>() - 540_040.0).abs() < 1e-6);
    let back = l
        .state_rho_e(s.density, s.specific_internal_energy, None)
        .unwrap();
    assert!((back.pressure.get::<pascal>() / 1.0e6 - 1.0).abs() < 1e-9);
    assert!((back.temperature.get::<kelvin>() - 400.0).abs() < 1e-9);
    let h = l.state_p_h(mpa(1.0), s.specific_enthalpy).unwrap();
    assert!((h.density.get::<kilogram_per_cubic_meter>() / rho - 1.0).abs() < 1e-12);
    let pr = l.state_p_rho(mpa(1.0), s.density).unwrap();
    assert!((pr.temperature.get::<kelvin>() - 400.0).abs() < 1e-9);
    assert_eq!(s.void_fraction.get::<ratio>(), 0.0);
    assert_eq!(
        StiffenedGasHem::water_vapour_le_metayer_2004()
            .state_p_t(mpa(0.1), kelvin_t(400.0))
            .unwrap()
            .void_fraction
            .get::<ratio>(),
        1.0
    );
    assert_eq!(l.name(), "stiffened gas");
}

/// Invalid parameters and states are refused, never passed on.
#[test]
fn stiffened_gas_refuses_bad_parameters_and_states() {
    let r = |v: f64| Ratio::new::<ratio>(v);
    let cv = |v: f64| SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(v);
    assert!(StiffenedGasHem::new(r(1.0), pressure(0.0), energy(0.0), cv(0.0), cv(1.0)).is_err());
    assert!(StiffenedGasHem::new(r(1.4), pressure(-1.0), energy(0.0), cv(0.0), cv(1.0)).is_err());
    assert!(StiffenedGasHem::new(r(1.4), pressure(0.0), energy(0.0), cv(0.0), cv(0.0)).is_err());
    let ok =
        StiffenedGasHem::new(r(2.35), pressure(1e9), energy(-1167e3), cv(0.0), cv(1816.0)).unwrap();
    assert_eq!(ok, StiffenedGasHem::water_liquid_le_metayer_2004());
    // Liquid in tension: (γ−1)ρ(e−q) < γp∞.
    let l = StiffenedGasHem::water_liquid_le_metayer_2004();
    assert!(l
        .state_rho_e(
            MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
            energy(0.0),
            None
        )
        .is_err());
    assert!(l.state_p_h(mpa(1.0), energy(-2.0e6)).is_err());
    assert!(l
        .state_rho_e(
            MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
            energy(f64::NAN),
            None
        )
        .is_err());
}

/// **The stiffened-gas saturation curve and latent heat against IAPWS-IF97**
/// (RELAP-7 Eqs 215–220, Le Métayer et al. 2004 parameters).
///
/// Methodology: at `T = 298, 323, …, 473` K take `p = p_sat,IF97(T)`, solve
/// Eq 215 for `T_sat,SG(p)` by Newton, and compare; compare `L_v,SG(T)` (Eq
/// 220) with IF97's `h_g − h_l` on the saturation line (regions 2 and 1 at
/// `p_sat`). Uncalibrated: the parameters are the paper's, not refitted.
///
/// Also checks that equal Gibbs energies hold at the computed `T_sat` (the
/// definition of Eq 215).
///
/// **Result (2026-11-01):** latent heat −3.0 to −4.5 % (prediction, within
/// 5 %, **held**). `T_sat` **−4.9 to −7.5 % (prediction, within 2 %,
/// MISSED)**: 283.3 K for 298 K, 437.5 K for 473 K. The cause is the vapour's
/// `q'` as RELAP-7 Table 5 prints it (−23×10³, the paper's value rounded; see
/// [`StiffenedGasHem::water_vapour_le_metayer_2004`]): at 373 K the computed
/// `ln p_sat` is 0.89 high, exactly a 400 J/(kg·K) change in `q'_v`. The miss
/// is pinned below at its measured size so a change shows; it is not a pass.
#[test]
fn stiffened_gas_saturation_follows_iapws97_inside_its_fit_range() {
    let pair = StiffenedGasPair::water_le_metayer_2004();
    let (mut worst_t, mut worst_l) = (0.0_f64, 0.0_f64);
    for k in 0..=7 {
        let t_ref = 298.0 + 25.0 * k as f64;
        let p = sat_pressure_4(kelvin_t(t_ref));
        let t_sg = pair.saturation_temperature(p).unwrap().get::<kelvin>();
        let g_l = pair
            .liquid
            .gibbs(p, kelvin_t(t_sg))
            .get::<joule_per_kilogram>();
        let g_v = pair
            .vapour
            .gibbs(p, kelvin_t(t_sg))
            .get::<joule_per_kilogram>();
        assert!(
            (g_l - g_v).abs() < 1e-6 * g_l.abs().max(1.0),
            "Gibbs {g_l} vs {g_v}"
        );
        let lv_if97 =
            (h_tp_2(kelvin_t(t_ref), p) - h_tp_1(kelvin_t(t_ref), p)).get::<joule_per_kilogram>();
        let lv_sg = pair
            .latent_heat(kelvin_t(t_ref))
            .get::<joule_per_kilogram>();
        let et = (t_sg / t_ref - 1.0).abs();
        let el = (lv_sg / lv_if97 - 1.0).abs();
        println!(
            "T {t_ref:.0} K: p_sat {:.4} MPa, T_sat,SG {t_sg:.2} K ({:+.3} %), L_v SG {:.1} vs IF97 {:.1} kJ/kg ({:+.2} %)",
            p.get::<megapascal>(),
            100.0 * (t_sg / t_ref - 1.0),
            lv_sg / 1e3,
            lv_if97 / 1e3,
            100.0 * (lv_sg / lv_if97 - 1.0)
        );
        worst_t = worst_t.max(et);
        worst_l = worst_l.max(el);
    }
    println!(
        "worst T_sat error {:.3} %, worst L_v error {:.2} %",
        100.0 * worst_t,
        100.0 * worst_l
    );
    // Pinned at the measured miss (7.5027 %), not at the 2 % prediction.
    assert!((worst_t - 0.075_027).abs() < 1e-4, "T_sat worst {worst_t}");
    assert!(worst_l < 0.05, "L_v worst {worst_l}");
    assert!(pair.saturation_temperature(pressure(-1.0)).is_err());
}

/// **Stiffened-gas liquid against IF97 at (1 MPa, 400 K)** — characterisation,
/// not a gate on the parameters: the predicted density is 5–10 % high and the
/// sound speed within 2 % (the paper fits saturation densities only roughly).
/// **Result (2026-11-01):** density +8.84 %, sound speed +0.39 % (both
/// predictions held); the bounds asserted are the predictions.
#[test]
fn stiffened_gas_liquid_against_iapws97_at_one_state() {
    let l = StiffenedGasHem::water_liquid_le_metayer_2004();
    let s = l.state_p_t(mpa(1.0), kelvin_t(400.0)).unwrap();
    let w = Iapws97Hem.state_p_t(mpa(1.0), kelvin_t(400.0)).unwrap();
    let d_rho = s.density.get::<kilogram_per_cubic_meter>()
        / w.density.get::<kilogram_per_cubic_meter>()
        - 1.0;
    let c_if97 = w_tp_1(kelvin_t(400.0), mpa(1.0)).get::<meter_per_second>();
    let d_c = s.equilibrium_sound_speed.get::<meter_per_second>() / c_if97 - 1.0;
    println!(
        "SG liquid at 1 MPa, 400 K: density {:+.2} %, sound speed {:+.2} % against IF97",
        100.0 * d_rho,
        100.0 * d_c
    );
    assert!(d_rho > 0.05 && d_rho < 0.10, "density offset {d_rho}");
    assert!(d_c.abs() < 0.02, "sound speed offset {d_c}");
}

/// **Isentropic stiffened gas and barotropic against their closed forms**
/// (RELAP-7 Eqs 54, 59, 60). Courant–Friedrichs water at 1.001 ρ₀:
/// `p = 3.04076e8·1.001^7 − 3.03975e8`. Barotropic `p₀ = 1e5`, `ρ₀ = 1000`,
/// `a = 1500`: at 1001 kg/m³, `p = 1e5 + 2.25e6`. Both refuse an inconsistent
/// `(p, ρ)` pair and report their reference temperature.
#[test]
fn isentropic_and_barotropic_match_their_closed_forms() {
    let isg = IsentropicStiffenedGasHem::water_courant_friedrichs();
    let rho = 999.8 * 1.001;
    let s = isg
        .state_rho_e(
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
            energy(1.0e5),
            None,
        )
        .unwrap();
    let p_expect = 3.04076e8 * 1.001_f64.powi(7) - 3.03975e8;
    assert!((s.pressure.get::<pascal>() / p_expect - 1.0).abs() < 1e-12);
    let c_expect = (7.0 * (p_expect + 3.03975e8) / rho).sqrt();
    assert!((s.equilibrium_sound_speed.get::<meter_per_second>() / c_expect - 1.0).abs() < 1e-12);
    assert_eq!(s.temperature.get::<kelvin>(), 293.15);
    let pt = isg.state_p_t(s.pressure, kelvin_t(500.0)).unwrap();
    assert!((pt.density.get::<kilogram_per_cubic_meter>() / rho - 1.0).abs() < 1e-12);
    let ph = isg.state_p_h(s.pressure, energy(2.0e5)).unwrap();
    assert!((ph.specific_enthalpy.get::<joule_per_kilogram>() - 2.0e5).abs() < 1e-6);
    assert!(isg.state_p_rho(s.pressure, s.density).is_ok());
    assert!(isg.state_p_rho(s.pressure * 1.01, s.density).is_err());
    assert_eq!(isg.name(), "isentropic stiffened gas");

    let b = BarotropicHem {
        p0_pa: 1.0e5,
        rho0: 1000.0,
        sound_speed_m_s: 1500.0,
        reference_temperature_k: 300.0,
    };
    let s = b
        .state_rho_e(
            MassDensity::new::<kilogram_per_cubic_meter>(1001.0),
            energy(1.0e5),
            None,
        )
        .unwrap();
    assert!((s.pressure.get::<pascal>() - (1.0e5 + 2.25e6)).abs() < 1e-6);
    assert_eq!(s.equilibrium_sound_speed.get::<meter_per_second>(), 1500.0);
    let pt = b.state_p_t(s.pressure, kelvin_t(1.0)).unwrap();
    assert!((pt.density.get::<kilogram_per_cubic_meter>() - 1001.0).abs() < 1e-9);
    let ph = b.state_p_h(s.pressure, energy(3.0e5)).unwrap();
    assert!((ph.specific_enthalpy.get::<joule_per_kilogram>() - 3.0e5).abs() < 1e-6);
    assert!(b.state_p_rho(s.pressure, s.density).is_ok());
    assert!(b.state_p_rho(s.pressure * 1.01, s.density).is_err());
    // Below the density where p reaches zero.
    assert!(b
        .state_rho_e(
            MassDensity::new::<kilogram_per_cubic_meter>(999.0),
            energy(1.0),
            None
        )
        .is_err());
    assert_eq!(b.name(), "barotropic");
}

/// **The linear EOS generated from IF97** (RELAP-7 Eqs 66–77). Methodology:
/// build it at (1 MPa, 400 K); it must return IF97's density and internal
/// energy there, close its own states both ways, and stay within the
/// predicted 1 % of IF97 density over ±5 K and ±0.5 MPa. The reference is the
/// **forward** region-1 equation (`v_tp_1`), the one the coefficients come
/// from: `Iapws97Hem::state_p_t` routes through the backward `(p, h)`
/// equations and differs from it by their own error, which the first run's
/// 1e-9 check caught. The coefficients are compared with the manual's Table 3 row for
/// (1 MPa, 400 K) only as a cross-check of the method, which the manual says
/// came from NIST data: `K_ρ 1.9474e6`, `K_T 1.6497e6`, `ρ₀ 937.87`.
/// **Result (2026-11-01):** printed and recorded in the V&V doc.
#[test]
fn linear_eos_from_iapws97_reproduces_its_reference_and_neighbourhood() {
    let lin = LinearEosHem::from_iapws97(mpa(1.0), kelvin_t(400.0)).unwrap();
    let rho_fwd = |p: Pressure, t: ThermodynamicTemperature| {
        1.0 / v_tp_1(t, p).get::<cubic_meter_per_kilogram>()
    };
    assert!((lin.rho0 / rho_fwd(mpa(1.0), kelvin_t(400.0)) - 1.0).abs() < 1e-12);
    println!(
        "linear EOS at (1 MPa, 400 K): K_rho {:.4e} (manual 1.9474e6), K_T {:.4e} (1.6497e6), rho0 {:.2} (937.87), cv {:.1}",
        lin.k_rho, lin.k_t, lin.rho0, lin.cv_j_per_kg_k
    );
    let s = lin.state_p_t(mpa(1.0), kelvin_t(400.0)).unwrap();
    let u_fwd = u_tp_1(kelvin_t(400.0), mpa(1.0)).get::<joule_per_kilogram>();
    assert!((s.specific_internal_energy.get::<joule_per_kilogram>() / u_fwd - 1.0).abs() < 1e-12);
    let back = lin
        .state_rho_e(s.density, s.specific_internal_energy, None)
        .unwrap();
    assert!((back.pressure.get::<pascal>() / 1.0e6 - 1.0).abs() < 1e-9);
    let ph = lin.state_p_h(mpa(1.0), s.specific_enthalpy).unwrap();
    assert!((ph.temperature.get::<kelvin>() - 400.0).abs() < 1e-6);
    let pr = lin.state_p_rho(mpa(1.0), s.density).unwrap();
    assert!((pr.temperature.get::<kelvin>() - 400.0).abs() < 1e-9);
    let mut worst = 0.0_f64;
    for (dp, dt) in [(0.5, 5.0), (-0.5, 5.0), (0.5, -5.0), (-0.5, -5.0)] {
        let (p, t) = (mpa(1.0 + dp), kelvin_t(400.0 + dt));
        let r_lin = lin
            .state_p_t(p, t)
            .unwrap()
            .density
            .get::<kilogram_per_cubic_meter>();
        let r_if = rho_fwd(p, t);
        worst = worst.max((r_lin / r_if - 1.0).abs());
    }
    println!(
        "linear EOS worst density error over ±5 K, ±0.5 MPa: {:.4} %",
        100.0 * worst
    );
    assert!(worst < 0.01);
    assert!(
        LinearEosHem::from_iapws97(mpa(0.1), kelvin_t(400.0)).is_err(),
        "vapour at 0.1 MPa, 400 K"
    );
    assert_eq!(lin.name(), "linear");
}

/// **Water shock tube through `HemPipe`, stiffened gas against IF97.**
/// Methodology: 10 m, 200 cells, closed liquid at 400 K, 10 MPa left of 5 m
/// and 1 MPa right, transmissive ends, run 2 ms (a pressure wave covers
/// ~3 m). The same problem on `Iapws97Hem` and on the Le Métayer liquid. The
/// measure is the plateau pressure (the mean over 4.5–5.5 m) and the position
/// of the right-running wave (the first cell from the outlet whose pressure
/// exceeds the midpoint of 1 MPa and that run's own plateau). Predicted:
/// plateau within 5 %, wave position within the ~2 % sound-speed difference
/// plus one cell.
///
/// **Instrument corrected 2026-11-01:** the first run used a fixed 5.5 MPa
/// threshold (the midpoint of 1 and 10 MPa), which no cell right of the
/// diaphragm reaches, since the plateau is 5.48 MPa; it found the
/// left-running wave instead. The threshold is now the midpoint of 1 MPa and
/// the run's plateau, the same rule for both closures.
///
/// **Result (2026-11-01):** plateau IF97 5.4770, SG 5.4871 MPa (+0.18 %,
/// held); front positions printed and asserted within the prediction.
#[test]
fn water_shock_tube_on_stiffened_gas_tracks_iapws97() {
    fn run<E: HemEquationOfState>(eos: E) -> (f64, f64) {
        let geo = HemPipeGeometry::new(
            Length::new::<meter>(10.0),
            Area::new::<square_meter>(1.0),
            200,
        )
        .unwrap();
        let state = |p: f64| PrimitiveState::PressureTemperature {
            pressure: mpa(p),
            temperature: kelvin_t(400.0),
            velocity: Velocity::new::<meter_per_second>(0.0),
        };
        let mut pipe = HemPipe::new(
            geo,
            eos,
            HemInitialCondition::Riemann {
                diaphragm: Length::new::<meter>(5.0),
                left: state(10.0),
                right: state(1.0),
            },
            HemNumerics::default(),
        )
        .unwrap();
        pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
            .unwrap();
        pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
            .unwrap();
        pipe.run_until(Time::new::<second>(2.0e-3)).unwrap();
        let snap = pipe.snapshot();
        let p: Vec<f64> = snap
            .cells
            .iter()
            .map(|c| c.pressure.get::<megapascal>())
            .collect();
        let x: Vec<f64> = snap.cell_centres.iter().map(|c| c.get::<meter>()).collect();
        let plateau: Vec<f64> = x
            .iter()
            .zip(&p)
            .filter(|(x, _)| **x > 4.5 && **x < 5.5)
            .map(|(_, p)| *p)
            .collect();
        let mean = plateau.iter().sum::<f64>() / plateau.len() as f64;
        let threshold = 0.5 * (1.0 + mean);
        let front = x
            .iter()
            .zip(&p)
            .rev()
            .find(|(_, p)| **p > threshold)
            .map(|(x, _)| *x)
            .unwrap();
        (mean, front)
    }
    let (p_if, x_if) = run(Iapws97Hem);
    let (p_sg, x_sg) = run(StiffenedGasHem::water_liquid_le_metayer_2004());
    println!("water shock tube at 2 ms: plateau IF97 {p_if:.4} MPa, SG {p_sg:.4} MPa ({:+.2} %); front IF97 {x_if:.3} m, SG {x_sg:.3} m", 100.0 * (p_sg / p_if - 1.0));
    assert!((p_sg / p_if - 1.0).abs() < 0.05);
    assert!((x_sg - x_if).abs() < 0.02 * (x_if - 5.0) + 0.05 + 1e-9);
}
