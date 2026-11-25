use super::*;
use crate::twin::{MeshResolution, TwinPage};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::degree_fahrenheit;

/// **gh:#1748: the plant on the template is the session, bit for bit.**
/// The demonstration transient (rods +10 pcm at 60 s, fuel pump trip at
/// 600 s, radiator doors to 1 % at 610 s) run to 700 s in 1 s steps,
/// once through the twin and once on a bare `Session` in the same
/// `MAX_ADVANCE_S` chunks. Every field of every snapshot must have the
/// same bits.
#[test]
fn the_twin_plant_is_the_session_bit_for_bit() {
    let mut twin = msre_twin(SessionConfig::default(), quick_split()).expect("twin");
    let mut bare = Session::new(SessionConfig::default()).expect("session");
    let script = demo_script();
    let mut next = 0;
    let dt = Time::new::<second>(1.0);
    for step in 0..700 {
        let t = Time::new::<second>(step as f64);
        while next < script.len() && script[next].0 <= t {
            twin.apply(script[next].1).expect("twin command");
            bare.apply(script[next].1).expect("bare command");
            next += 1;
        }
        let a = twin.advance(dt).expect("twin step").to_vec();
        let b = bare.advance(1.0).expect("bare step").to_vec();
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&a), bits(&b), "step {step}");
    }
    assert_eq!(next, 3, "all three events applied");
    // And the template's own CSV of it is deterministic.
    let csv = |_: ()| msre_twin(SessionConfig::default(), quick_split()).unwrap().plant_csv(&script, dt, Time::new::<second>(700.0)).unwrap();
    let (x, y) = (csv(()), csv(()));
    assert_eq!(x, y);
    assert_eq!(x.lines().next(), Some(Snapshot::FIELDS.join(",").as_str()));
    assert_eq!(x.lines().count(), 1 + 1 + 700);
}

/// The CV map is read from the live plant: no dangling links, the core
/// lumps carry the model's own temperatures, the design fuel mass flow
/// times the TUAS `c_p` is `MsreDesignPoint::fuel_heat_capacity_rate`
/// (the same evaluation, to round-off), and a fuel pump trip shows in the
/// fuel flow. No volume or mass is made up.
#[test]
fn the_cv_map_is_the_live_plant() {
    let mut twin = msre_twin(SessionConfig::default(), quick_split()).expect("twin");
    let cvs = twin.control_volumes();
    assert!(cvs.dangling_links().is_empty());
    assert_eq!(cvs.volumes.len(), 10);
    assert!(cvs.volumes.iter().all(|v| v.volume.is_none() && v.mass.is_none()));
    let s = twin.plant().snapshot();
    let t = |id: &str| cvs.get(id).and_then(|v| v.temperature).map(|t| t.get::<degree_fahrenheit>()).unwrap();
    assert!((t("fuel.core") - s.fuel_f).abs() < 1e-9 && (t("graphite.core") - s.graphite_f).abs() < 1e-9);
    assert_eq!(cvs.get("fuel.core").map(|v| v.role), Some(CvRole::Balance));
    assert_eq!(cvs.get("fuel.hx_inlet").map(|v| v.on_pressure_boundary), Some(true));
    assert_eq!(cvs.get("coolant.hx_inlet").map(|v| v.on_pressure_boundary), Some(false));

    let fuel_flow = |c: &ControlVolumes| match c.links[0].kind {
        CvLinkKind::Flow(m) => m.get::<kilogram_per_second>(),
        CvLinkKind::Heat(_) => panic!("link 0 is the fuel flow"),
    };
    let m0 = fuel_flow(&cvs);
    let d = MsreDesignPoint::tm0728();
    let t_mid = ThermodynamicTemperature::new::<degree_fahrenheit>(1200.0);
    let cp = tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_fuel_salt_tm0728::msre_fuel_salt_cp(t_mid);
    let c_rate = (MassRate::new::<kilogram_per_second>(m0 / s.fuel_flow) * cp).get::<watt_per_kelvin>();
    let want = d.fuel_heat_capacity_rate().get::<watt_per_kelvin>();
    assert!(((c_rate - want) / want).abs() < 1e-12, "{c_rate} against {want}");
    println!("MSRE fuel loop at full flow: {:.1} kg/s", m0 / s.fuel_flow);

    twin.apply(Command::TripFuelPump).expect("trip");
    twin.advance(Time::new::<second>(30.0)).expect("coast down");
    let later = twin.control_volumes();
    assert!(fuel_flow(&later) < 0.5 * m0, "{} against {m0}", fuel_flow(&later));
    let text = twin.dump_cvs(0, Time::new::<second>(1.0)).expect("dump");
    assert!(text.contains("fuel.core,Core fuel-salt lump,fuel salt,balance,,true,"), "{text}");
}

/// The coefficient sources: the default session is on the cited 1965 set,
/// the N-3 session on the committed record; switching the source switches
/// the session's coefficients; the multiphysics pages name gh:#1743.
#[test]
fn the_sources_and_pages_are_named() {
    let mut twin = msre_twin(SessionConfig::default(), quick_split()).expect("twin");
    assert_eq!(twin.coefficients().provenance(), format!("cited: {ORNL_1965_CITATION}"));
    assert_eq!(twin.plant().session().config().coefficients, Coefficients::Ornl1965);
    twin.use_coefficients(CoefficientSource::Committed { value: Coefficients::MonteCarloN3, record: N3_RECORD.into() }).expect("switch");
    assert_eq!(twin.plant().session().config().coefficients, Coefficients::MonteCarloN3);

    let n3 = SessionConfig { coefficients: Coefficients::MonteCarloN3, ..SessionConfig::default() };
    let twin3 = msre_twin(n3, quick_split()).expect("twin");
    assert!(twin3.coefficients().provenance().starts_with("committed Monte Carlo record: crates/nee_soon/"));
    let m = Coefficients::Measured { fuel_per_f: -4e-5, fuel_sigma: 1e-6, graphite_per_f: -3e-5, graphite_sigma: 1e-6 };
    let twin_m = msre_twin(SessionConfig { coefficients: m, ..SessionConfig::default() }, quick_split()).expect("twin");
    assert_eq!(*twin_m.coefficients().value(), m);

    assert_eq!(twin.page_status(TwinPage::MonteCarlo), Ok(()));
    assert_eq!(twin.page_status(TwinPage::CoarseMultiphysics), Err(TwinError::NotBuilt { page: TwinPage::CoarseMultiphysics, issue: 1743 }));
    assert!(twin.run_multiphysics(MeshResolution::Fine).is_err());
    assert!(matches!(MsrePlant::build(SessionConfig { power_mw: -1.0, ..SessionConfig::default() }), Err(TwinError::Plant(_))));
}

/// The split's runs are A, C, D at the record's temperatures; its
/// coefficients are `coefficient_from_pair` on A-C and A-D, the same
/// arithmetic as the template's SI `reactivity_coefficient` (×1.8).
#[test]
fn the_split_is_the_k_eff_tabs() {
    let mut sp = MsreSplit { shield: false, u235_scale: 1.02, ..quick_split() };
    let runs = sp.runs();
    assert_eq!(runs.len(), 3);
    assert!(runs[0].label.starts_with("A:") && runs[1].label.starts_with("C:") && runs[2].label.starts_with("D:"));
    assert_eq!((sp.study(0).salt_f, sp.study(0).graphite_f), (SPLIT_BASE_F, SPLIT_BASE_F));
    assert_eq!((sp.study(1).salt_f, sp.study(1).graphite_f), (SPLIT_BASE_F + SPLIT_STEP_F, SPLIT_BASE_F));
    assert_eq!((sp.study(2).salt_f, sp.study(2).graphite_f), (SPLIT_BASE_F, SPLIT_BASE_F + SPLIT_STEP_F));
    assert!(!sp.study(1).shield && sp.study(2).u235_scale == 1.02);

    let ks = [KeffResult { k: 1.02232, sigma: 3e-4 }, KeffResult { k: 1.01837, sigma: 3e-4 }, KeffResult { k: 1.01900, sigma: 3e-4 }];
    let c = sp.coefficients(&ks).expect("three runs");
    let (f, sf) = coefficient_from_pair(ks[0].k, ks[0].sigma, ks[1].k, ks[1].sigma, SPLIT_STEP_F);
    let (g, sg) = coefficient_from_pair(ks[0].k, ks[0].sigma, ks[2].k, ks[2].sigma, SPLIT_STEP_F);
    assert_eq!(c, Coefficients::Measured { fuel_per_f: f, fuel_sigma: sf, graphite_per_f: g, graphite_sigma: sg });
    let si = crate::twin::reactivity_coefficient(ks[0], ks[1], runs[1].delta).value.get::<per_kelvin>();
    assert!(((si - 1.8 * f) / si).abs() < 1e-12, "{si} against {}", 1.8 * f);
    assert_eq!(sp.describe(&c), vec![c.describe()]);
    assert!(sp.coefficients(&ks[..2]).is_err());
    assert!(sp.run(3).is_err());
}

/// **gh:#1748: the live split on the template, natively.** The quickest
/// preset (4000 × [30 + 140], seed 1, whole shield) runs A, C and D through
/// `MsreSplit::run` and the template's `coefficient_report`, which sends
/// the result to the plant. Prints the report. Not a V&V result: the
/// committed N-3 record is the number to quote.
#[test]
#[ignore = "processes the MSRE model's tapes three times and runs three full-core k_eff (many minutes); run with --ignored --nocapture"]
fn the_live_split_reaches_the_plant() {
    let mut twin = msre_twin(SessionConfig::default(), quick_split()).expect("twin");
    let report = twin.coefficient_report().expect("report");
    println!("{report}");
    assert!(matches!(twin.coefficients().value(), Coefficients::Measured { .. }));
    assert!(matches!(twin.plant().session().config().coefficients, Coefficients::Measured { .. }));
}
