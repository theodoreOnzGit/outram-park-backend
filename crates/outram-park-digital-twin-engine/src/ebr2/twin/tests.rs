use super::*;
use crate::ebr2::shrt::ShrtTest;
use crate::twin::{MeshResolution, TwinPage};

/// **gh:#1749: the plant on the template is the session, bit for bit.**
/// SHRT-45R (the default) and SHRT-17, 300 s in 1 s steps, once through
/// the twin and once on a bare `Session`, with an external +5 pcm at
/// 100 s on SHRT-45R. Every field of every snapshot must have the same
/// bits.
#[test]
fn the_twin_plant_is_the_session_bit_for_bit() {
    for test in [ShrtTest::Shrt45r, ShrtTest::Shrt17] {
        let config = SessionConfig { test, ..SessionConfig::default() };
        let mut twin = ebr2_twin(config).expect("twin");
        let mut bare = Session::new(config).expect("session");
        for step in 0..300 {
            if step == 100 && test == ShrtTest::Shrt45r {
                twin.apply(Command::ExternalPcm(5.0)).expect("twin rod");
                bare.apply(Command::ExternalPcm(5.0)).expect("bare rod");
            }
            let a = twin.advance(Time::new::<second>(1.0)).expect("twin step").to_vec();
            let b = bare.advance(1.0).expect("bare step").to_vec();
            let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
            assert_eq!(bits(&a), bits(&b), "{test:?} step {step}");
        }
    }
    let csv = |_: ()| ebr2_twin(SessionConfig::default()).unwrap().plant_csv(&demo_script(), Time::new::<second>(1.0), Time::new::<second>(60.0)).unwrap();
    assert_eq!(csv(()), csv(()));
    assert_eq!(csv(()).lines().count(), 1 + 1 + 60);
}

/// The CV map is the live plant: four balance nodes with the model's own
/// temperatures and sodium masses, no dangling link, the primary loop at
/// the model's flow, which falls as the pumps coast down.
#[test]
fn the_cv_map_is_the_live_plant() {
    let mut twin = ebr2_twin(SessionConfig::default()).expect("twin");
    let cvs = twin.control_volumes();
    assert!(cvs.dangling_links().is_empty());
    let s = twin.plant().snapshot();
    let t = |id: &str| cvs.get(id).and_then(|v| v.temperature).map(|t| t.get::<degree_celsius>()).unwrap();
    assert!((t("core.fuel") - s.fuel_c).abs() < 1e-9 && (t("primary.pool") - s.core_inlet_c).abs() < 1e-9);
    let m = |id: &str| cvs.get(id).and_then(|v| v.mass).map(|m| m.get::<kilogram>()).unwrap();
    assert!((m("primary.core") - s.core_na_kg).abs() < 1e-9 && (m("primary.pool") - s.pool_na_kg).abs() < 1e-9);
    assert_eq!(cvs.volumes.iter().filter(|v| v.role == CvRole::Balance).count(), 4);
    let flow0 = match cvs.links[0].kind {
        CvLinkKind::Flow(m) => m.get::<kilogram_per_second>(),
        CvLinkKind::Heat(_) => panic!("link 0 is the primary flow"),
    };
    assert!((flow0 - s.primary_flow_kg_s).abs() < 1e-9);
    let text = twin.dump_cvs(60, Time::new::<second>(1.0)).expect("dump");
    let later = twin.plant().snapshot();
    assert!(later.primary_flow_kg_s < 0.5 * s.primary_flow_kg_s, "the SHRT-45R pumps have coasted down");
    assert!(text.contains(&format!("primary.pool,Primary pool (core inlet),sodium,balance,,true,{:.3},", later.core_inlet_c + 273.15)), "{text}");
}

/// The coefficients: cited Argonne VARIANT in use; a new set reaches the
/// model through the twin and the session reports it; SHRT-17 (tabulated)
/// keeps the cited set and refuses another. The missing pages name their
/// issues, and the Monte Carlo run is refused with gh:#1313.
#[test]
fn the_sources_and_pages_are_named() {
    let mut twin = ebr2_twin(SessionConfig::default()).expect("twin");
    let argonne = ReactivityCoefficients::argonne(ArgonneSolver::Variant);
    assert_eq!(twin.coefficients().provenance(), format!("cited: {ARGONNE_CITATION}"));
    assert_eq!(twin.plant().session().reactivity_coefficients(), Some(argonne));
    let doubled = ReactivityCoefficients { doppler_per_kelvin: argonne.doppler_per_kelvin * 2.0, ..argonne };
    twin.use_coefficients(CoefficientSource::Committed { value: doubled, record: "test".into() }).expect("switch");
    assert_eq!(twin.plant().session().reactivity_coefficients(), Some(doubled));

    let mut t17 = ebr2_twin(SessionConfig { test: ShrtTest::Shrt17, ..SessionConfig::default() }).expect("SHRT-17 twin");
    assert_eq!(t17.plant().session().reactivity_coefficients(), None);
    assert!(matches!(t17.use_coefficients(CoefficientSource::Cited { value: doubled, citation: "x".into() }), Err(TwinError::Plant(_))));
    assert_eq!(*t17.coefficients().value(), argonne);

    assert_eq!(twin.page_status(TwinPage::MonteCarlo), Err(TwinError::NotBuilt { page: TwinPage::MonteCarlo, issue: 1313 }));
    assert_eq!(twin.page_status(TwinPage::FineMultiphysics), Err(TwinError::NotBuilt { page: TwinPage::FineMultiphysics, issue: 1747 }));
    assert!(twin.run_multiphysics(MeshResolution::Coarse).is_err());
    assert_eq!(twin.coefficient_report().unwrap_err().to_string(), "Monte Carlo is not built yet (gh:#1313)");
    assert!(matches!(Ebr2Plant::build(SessionConfig { thermal_centre_height_m: -1.0, ..SessionConfig::default() }), Err(TwinError::Plant(_))));
}
