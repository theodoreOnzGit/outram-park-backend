//! Tests of [`super`] (gh:#1326). Every input here is generic and
//! illustrative (the GeN-Foam tutorial's own feature-demonstration values
//! for the driveline map, otherwise round numbers); none is a reactor's
//! evaluated coefficient.

use super::*;
use crate::feedback_mechanisms::separable::{
    FuelHeatStructure, ModeratorState, ModeratorVolume, SeparableFeedback,
};
use uom::si::time::second;

fn r(x: f64) -> Ratio {
    Ratio::new::<ratio>(x)
}
fn k(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}
fn pk(x: f64) -> TemperatureCoefficient {
    TemperatureCoefficient::new::<per_kelvin>(x)
}
fn d(x: f64) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(x)
}
fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn sv(x: f64) -> CoolantDensityCoefficient {
    SpecificVolume::new::<cubic_meter_per_kilogram>(x)
}
fn lin(a: f64, t_ref: f64) -> LinearTemperatureTerm {
    LinearTemperatureTerm {
        coefficient: pk(a),
        reference_temperature: k(t_ref),
    }
}

/// The GeN-Foam feature tutorial's driveline inputs
/// (`Tutorials/featureCases/2D_onePhaseAndPointKineticsCoupling/rootCase/
/// constant/neutroRegion/nuclearData`, commit 652b3da):
/// `absoluteDrivelineExpansionCoeff 8e-05` m/K and
/// `controlRodReactivityMap ((0.1 -0.01) (0 0) (-0.1 0.01))`, which its
/// README calls 100 pcm/cm, "arbitrarily set ... only for feature
/// demonstration purposes". Here `L = 1 m`, `alpha = 8e-5 /K`.
fn tutorial_driveline() -> DrivelineExpansionTerm {
    DrivelineExpansionTerm {
        length: m(1.0),
        linear_expansion_coefficient: pk(8e-5),
        reference_temperature: k(700.0),
        worth: RodWorthMap::new(
            vec![m(0.1), m(0.0), m(-0.1)],
            vec![r(-0.01), r(0.0), r(0.01)],
        )
        .unwrap(),
    }
}

fn state(
    t_fuel: f64,
    t_clad: f64,
    t_cool: f64,
    rho: f64,
    structs: &[f64],
    t_dl: f64,
) -> FastReactorState {
    FastReactorState {
        fuel_temperature: k(t_fuel),
        cladding_temperature: k(t_clad),
        coolant_temperature: k(t_cool),
        coolant_density: d(rho),
        radial_structure_temperatures: structs.iter().map(|t| k(*t)).collect(),
        driveline_temperature: k(t_dl),
    }
}

/// **V&V: each linear term is coefficient x deviation, by hand.**
///
/// *Methodology.* Axial fuel `a = -3e-6 /K`, ref 800 K, fuel at 900 K;
/// cladding `-1e-6`, 700 K -> 750 K; coolant temperature `-2e-6`, 650 K ->
/// 700 K; coolant density `a_rho = -4e-5 m3/kg` (positive void worth), 850
/// -> 830 kg/m3; two radial structures (grid plate, load pad) `-5e-6` and
/// `-2e-6` /K, refs 620 and 700 K, at 640 and 720 K.
/// *Prediction.* Hand: -3e-4, -5e-5, -1e-4, +8e-4, -1e-4 - 4e-5 = -1.4e-4;
/// each to 1e-18 (one product of exact-ish binary values, one rounding).
/// *Result (2026-11-10):* passes.
#[test]
fn each_linear_term_is_coefficient_times_deviation() {
    let fb = FastReactorFeedback {
        axial_fuel_expansion: Some(lin(-3e-6, 800.0)),
        cladding: Some(lin(-1e-6, 700.0)),
        coolant_temperature: Some(lin(-2e-6, 650.0)),
        coolant_density: Some(CoolantDensityTerm {
            coefficient: sv(-4e-5),
            reference_density: d(850.0),
        }),
        radial_expansion: vec![lin(-5e-6, 620.0), lin(-2e-6, 700.0)],
        ..Default::default()
    };
    let got = fb
        .evaluate(&state(900.0, 750.0, 700.0, 830.0, &[640.0, 720.0], 0.0))
        .unwrap();
    for (name, value, want) in [
        ("axial", got.axial_fuel_expansion, -3e-4),
        ("clad", got.cladding, -5e-5),
        ("coolant T", got.coolant_temperature, -1e-4),
        ("coolant rho", got.coolant_density, 8e-4),
        ("radial", got.radial_expansion, -1.4e-4),
    ] {
        assert!(
            (value.get::<ratio>() - want).abs() < 1e-18,
            "{name}: {value:?} vs {want}"
        );
    }
    assert_eq!(got.doppler, Ratio::ZERO);
    assert_eq!(got.driveline, Ratio::ZERO);
    let total = -3e-4 - 5e-5 - 1e-4 + 8e-4 - 1.4e-4;
    assert!((got.total().get::<ratio>() - total).abs() < 1e-18);
    assert!((got.total_in_dollars(r(0.0035)).get::<ratio>() - total / 0.0035).abs() < 1e-15);
}

/// **V&V: sign conventions.** Heating with negative coefficients gives
/// negative reactivity and cooling positive; losing sodium density with a
/// negative `a_rho` (positive void worth) adds reactivity; a positive
/// Doppler constant is stabilising above `T_ref` and destabilising below;
/// with the tutorial's map, a driveline that heats (and grows) inserts the
/// rods (negative) and one that cools withdraws them (positive).
/// *Prediction:* every sign as stated. *Result (2026-11-10):* passes.
#[test]
fn sign_conventions() {
    let a = lin(-3e-6, 800.0);
    assert!(a.reactivity(k(900.0)).get::<ratio>() < 0.0);
    assert!(a.reactivity(k(700.0)).get::<ratio>() > 0.0);
    assert_eq!(a.reactivity(k(800.0)).get::<ratio>(), 0.0);

    let void = CoolantDensityTerm {
        coefficient: sv(-4e-5),
        reference_density: d(850.0),
    };
    assert!(void.reactivity(d(800.0)).get::<ratio>() > 0.0);

    let dop = FastDopplerTerm {
        doppler_constant: r(5e-3),
        reference_temperature: k(800.0),
    };
    assert!(dop.reactivity(k(1000.0)).get::<ratio>() < 0.0);
    assert!(dop.reactivity(k(600.0)).get::<ratio>() > 0.0);

    let dl = tutorial_driveline();
    assert!(dl.expansion(k(750.0)).get::<meter>() > 0.0);
    assert!(dl.reactivity(k(750.0)).get::<ratio>() < 0.0);
    assert!(dl.reactivity(k(650.0)).get::<ratio>() > 0.0);
}

/// **V&V: fast Doppler against its closed form.**
///
/// *Methodology.* `K_D = 5e-3` (the GeN-Foam FFTF tutorial's
/// `feedbackCoeffDoppler`, an illustrative value), `T_ref = 800 K`. At
/// `T = T_ref e` the term is `-K_D` exactly in real arithmetic; at `T_ref`
/// it is zero; its secant slope over +-0.01 K at `T_ref` is `-K_D/T_ref`
/// (the linear coefficient it reduces to); at `T = 0` and `T_ref = 0` the
/// `SMALL` floor keeps it finite, as upstream.
/// *Prediction.* `-K_D` to 1e-17 (one rounding in `e` and in `ln`);
/// `+0.0` at `T_ref`; slope to 1e-9 relative (secant error `O(h^2/T^2)`
/// ~1e-10, cancellation ~1e-9); finite at zero.
/// *Result (2026-11-10):* passes.
#[test]
fn fast_doppler_closed_form() {
    let dop = FastDopplerTerm {
        doppler_constant: r(5e-3),
        reference_temperature: k(800.0),
    };
    let e = 1.0_f64.r_exp();
    assert!((dop.reactivity(k(800.0 * e)).get::<ratio>() + 5e-3).abs() < 1e-17);
    assert_eq!(dop.reactivity(k(800.0)).get::<ratio>(), 0.0);
    let h = 0.01;
    let slope = (dop.reactivity(k(800.0 + h)).get::<ratio>()
        - dop.reactivity(k(800.0 - h)).get::<ratio>())
        / (2.0 * h);
    assert!(
        ((slope - (-5e-3 / 800.0)) / (5e-3 / 800.0)).abs() < 1e-9,
        "{slope}"
    );
    assert!(dop.reactivity(k(0.0)).get::<ratio>().is_finite());
    let at_zero_ref = FastDopplerTerm {
        doppler_constant: r(5e-3),
        reference_temperature: k(0.0),
    };
    assert!(at_zero_ref.reactivity(k(300.0)).get::<ratio>().is_finite());
}

/// **V&V (differential, against upstream's algorithm and inputs): the
/// driveline term.**
///
/// *Methodology.* No GeN-Foam build exists in this workspace (it needs
/// OpenFOAM), so the reference is `calcDrivelineReactivity`
/// (`pointKineticNeutronics.C:998-1061`, commit 652b3da) evaluated by hand
/// on the feature tutorial's own inputs ([`tutorial_driveline`]). On the
/// descending map the first interval is `(0.1, -0.01) -> (0, 0)` with slope
/// `m = (0 - (-0.01)) / (0 - 0.1) = -0.1`, so
/// `rho = -0.1 (dL - 0.1) - 0.01 = -0.1 dL`, and likewise on the second.
/// - `dT = +100 K`: `dL = 8e-3 m`, `rho = -8e-4`.
/// - `dT = -250 K`: `dL = -0.02 m`, `rho = +2e-3`.
/// - `dL = 0.1` (first point of the descending map): in the first interval
///   (`x <= x0`), `rho = -0.01`.
/// - `dL = -0.1` (last point): no interval holds it (`x > x1` fails), so
///   upstream returns **0**, not 0.01.
/// - `dL = 0.15` or `-0.15` (outside): upstream returns **0**.
/// - The same map in ascending order agrees inside; at its first point
///   (`x0 = -0.1`, excluded) it gives 0, and at its last (0.1) -0.01.
///
/// *Prediction.* Inside the map, equal to the hand value to 1e-18; the
/// edge and outside values exactly zero, as upstream.
/// *Result (2026-11-10):* passes. The zeros at and beyond the map's ends
/// are upstream's behaviour and are kept (see the module docs).
#[test]
fn driveline_matches_upstream_algorithm_on_tutorial_inputs() {
    let dl = tutorial_driveline();
    assert!((dl.expansion(k(800.0)).get::<meter>() - 8e-3).abs() < 1e-18);
    assert!((dl.reactivity(k(800.0)).get::<ratio>() + 8e-4).abs() < 1e-18);
    assert!((dl.reactivity(k(450.0)).get::<ratio>() - 2e-3).abs() < 1e-18);
    let map = &dl.worth;
    assert!((map.reactivity(m(0.1)).get::<ratio>() + 0.01).abs() < 1e-18);
    assert_eq!(map.reactivity(m(-0.1)).get::<ratio>(), 0.0);
    assert!(!map.covers(m(-0.1)));
    for out in [0.15, -0.15] {
        assert_eq!(map.reactivity(m(out)).get::<ratio>(), 0.0, "{out}");
        assert!(!map.covers(m(out)));
    }
    let ascending = RodWorthMap::new(
        vec![m(-0.1), m(0.0), m(0.1)],
        vec![r(0.01), r(0.0), r(-0.01)],
    )
    .unwrap();
    for x in [-0.07, -0.02, 0.0, 0.008, 0.09] {
        let (a, b) = (
            ascending.reactivity(m(x)).get::<ratio>(),
            map.reactivity(m(x)).get::<ratio>(),
        );
        assert!(
            (a - b).abs() < 1e-18 && (a + 0.1 * x).abs() < 1e-18,
            "{x}: {a} {b}"
        );
    }
    assert_eq!(ascending.reactivity(m(-0.1)).get::<ratio>(), 0.0);
    assert!((ascending.reactivity(m(0.1)).get::<ratio>() + 0.01).abs() < 1e-18);
}

/// **V&V: the linear fast terms are the separable model's linear terms,
/// bit for bit, and the two models compose by addition.**
///
/// *Methodology.* Axial fuel expansion `a = -2.7e-6 /K` (ref 761.3 K) is the
/// same form as the separable model's `a_F dT_F` with `a_F = a`; coolant
/// temperature `a = -1.9e-6` (ref 655.1 K) is its `a_M dT_M`. Both models
/// are evaluated at three states. Then a separable model with a scram curve
/// and those linear terms, and a fast set with every term, are composed.
/// *Prediction.* Bitwise equality of the corresponding terms (same
/// operations), except the sign of an exact zero; the composed total is
/// `separable.total() + fast.total()` bitwise, and equals the hand sum of
/// all twelve parts to 1e-18.
/// *Result (2026-11-10):* passes.
#[test]
fn linear_terms_match_separable_and_compose() {
    let fuel = lin(-2.7e-6, 761.3);
    let cool = lin(-1.9e-6, 655.1);
    let sep = SeparableFeedback {
        fuel_structures: vec![FuelHeatStructure {
            temperature_weight: r(0.0),
            temperature_coefficient: fuel.coefficient,
            initial_temperature: fuel.reference_temperature,
        }],
        moderator_volumes: vec![ModeratorVolume {
            density_weight: r(0.0),
            temperature_coefficient: cool.coefficient,
            initial_temperature: cool.reference_temperature,
        }],
        ..Default::default()
    };
    let fast = FastReactorFeedback {
        axial_fuel_expansion: Some(fuel),
        coolant_temperature: Some(cool),
        ..Default::default()
    };
    for (tf, tc) in [(761.3, 655.1), (1043.77, 702.4), (512.9, 600.03)] {
        let s = sep
            .evaluate(
                Time::new::<second>(0.0),
                &[ModeratorState {
                    density: d(850.0),
                    temperature: k(tc),
                }],
                &[k(tf)],
            )
            .unwrap();
        let f = fast.evaluate(&state(tf, 0.0, tc, 850.0, &[], 0.0)).unwrap();
        let (sf, ff) = (
            s.fuel_temperature.get::<ratio>(),
            f.axial_fuel_expansion.get::<ratio>(),
        );
        let (sc, fc) = (
            s.moderator_temperature.get::<ratio>(),
            f.coolant_temperature.get::<ratio>(),
        );
        if sf != 0.0 {
            assert_eq!(sf.to_bits(), ff.to_bits(), "fuel at {tf}");
            assert_eq!(sc.to_bits(), fc.to_bits(), "coolant at {tc}");
        } else {
            // the separable sum starts from +0.0; a negative coefficient
            // times +0.0 is -0.0 here. Equal values.
            assert_eq!(sf, ff);
            assert_eq!(sc, fc);
        }
    }

    // Composition.
    let sep_full = SeparableFeedback {
        scram_curves: vec![
            crate::feedback_mechanisms::separable::ScramReactivityCurve::new(
                vec![Time::new::<second>(0.0), Time::new::<second>(1.0)],
                vec![r(0.0), r(-0.02)],
            )
            .unwrap(),
        ],
        ..sep.clone()
    };
    let fast_full = FastReactorFeedback {
        doppler: Some(FastDopplerTerm {
            doppler_constant: r(5e-3),
            reference_temperature: k(761.3),
        }),
        cladding: Some(lin(-1e-6, 700.0)),
        coolant_density: Some(CoolantDensityTerm {
            coefficient: sv(-4e-5),
            reference_density: d(850.0),
        }),
        radial_expansion: vec![lin(-5e-6, 620.0), lin(-2e-6, 700.0)],
        driveline: Some(tutorial_driveline()),
        // the fuel and coolant temperature terms stay in the separable
        // model here, so they are not counted twice
        ..Default::default()
    };
    let s = sep_full
        .evaluate(
            Time::new::<second>(0.5),
            &[ModeratorState {
                density: d(830.0),
                temperature: k(680.0),
            }],
            &[k(900.0)],
        )
        .unwrap();
    let f = fast_full
        .evaluate(&state(900.0, 750.0, 680.0, 830.0, &[640.0, 720.0], 800.0))
        .unwrap();
    let composed = f.total_with_separable(&s);
    assert_eq!(
        composed.get::<ratio>().to_bits(),
        (s.total() + f.total()).get::<ratio>().to_bits()
    );
    let hand = -0.01                       // scram at half its 1 s ramp
        + -2.7e-6 * (900.0 - 761.3)       // separable a_F dT_F
        + -1.9e-6 * (680.0 - 655.1)       // separable a_M dT_M
        + -5e-3 * (900.0_f64 / 761.3).r_ln() // fast Doppler
        + -5e-5                           // clad
        + -4e-5 * (830.0 - 850.0)         // coolant density
        + -1.4e-4                         // radial
        + -8e-4; // driveline, dT = 100 K
    assert!(
        (composed.get::<ratio>() - hand).abs() < 1e-17,
        "{composed:?} vs {hand}"
    );
}

/// Validation: a map of fewer than two points, unequal lengths, a
/// non-monotonic axis or a non-finite value is refused, and the state must
/// carry one temperature per radial structure.
#[test]
fn inputs_are_validated() {
    assert!(RodWorthMap::new(vec![m(0.0)], vec![r(0.0)]).is_err());
    assert!(RodWorthMap::new(vec![m(0.0), m(1.0)], vec![r(0.0)]).is_err());
    assert!(RodWorthMap::new(vec![m(0.0), m(1.0), m(0.5)], vec![r(0.0); 3]).is_err());
    assert!(RodWorthMap::new(vec![m(0.0), m(0.0)], vec![r(0.0); 2]).is_err());
    assert!(RodWorthMap::new(vec![m(0.0), m(f64::NAN)], vec![r(0.0); 2]).is_err());
    let fb = FastReactorFeedback {
        radial_expansion: vec![lin(-5e-6, 620.0)],
        ..Default::default()
    };
    assert!(fb.evaluate(&state(0.0, 0.0, 0.0, 0.0, &[], 0.0)).is_err());
    let empty = FastReactorFeedback::default()
        .evaluate(&state(900.0, 800.0, 700.0, 800.0, &[], 900.0))
        .unwrap();
    assert_eq!(empty.total(), Ratio::ZERO);
}
