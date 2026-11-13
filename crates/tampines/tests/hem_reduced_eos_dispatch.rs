//! # The reduced equations of state through `tampines::hem::HemEos` (gh:#1057)
//!
//! Methodology: every reduced closure (stiffened gas, isentropic stiffened
//! gas, barotropic, linear) is wrapped in the run-time enum [`HemEos`] and must
//! give **bit-identical** states to the closure called directly, through every
//! trait method; one water shock tube is run through `tampines::hem::HemPipe`
//! (the enum-dispatched solver) and must match the same run on the concrete
//! closure byte for byte. Enum dispatch adds no arithmetic, so any difference
//! is a wiring defect.
//!
//! Result (2026-11-01): identical in every case (the asserts are `==`).
//! Physics V&V of the closures themselves is in `tampines-steam-tables`
//! (`hem::reduced_eos::tests`).

use tampines::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::megapascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

fn closures() -> Vec<HemEos> {
    vec![
        HemEos::StiffenedGas(StiffenedGasHem::water_liquid_le_metayer_2004()),
        HemEos::IsentropicStiffenedGas(IsentropicStiffenedGasHem::water_courant_friedrichs()),
        HemEos::Barotropic(BarotropicHem {
            p0_pa: 1.0e6,
            rho0: 1000.0,
            sound_speed_m_s: 1500.0,
            reference_temperature_k: 300.0,
        }),
        HemEos::Linear(
            LinearEosHem::from_iapws97(
                Pressure::new::<megapascal>(1.0),
                ThermodynamicTemperature::new::<kelvin>(400.0),
            )
            .unwrap(),
        ),
    ]
}

/// Call every trait method on the enum and on the closure it wraps.
fn same<E: HemEquationOfState>(wrapped: &HemEos, direct: &E) {
    let p = Pressure::new::<megapascal>(1.0);
    let t = ThermodynamicTemperature::new::<kelvin>(400.0);
    assert_eq!(wrapped.name(), direct.name());
    let a = wrapped.state_p_t(p, t).unwrap();
    assert_eq!(a, direct.state_p_t(p, t).unwrap());
    assert_eq!(
        wrapped.state_p_h(p, a.specific_enthalpy).ok(),
        direct.state_p_h(p, a.specific_enthalpy).ok()
    );
    assert_eq!(
        wrapped.state_p_rho(p, a.density).ok(),
        direct.state_p_rho(p, a.density).ok()
    );
    assert_eq!(
        wrapped
            .state_rho_e(a.density, a.specific_internal_energy, Some(p))
            .unwrap(),
        direct
            .state_rho_e(a.density, a.specific_internal_energy, Some(p))
            .unwrap()
    );
    assert!(wrapped.break_flow(p * 2.0, a.specific_enthalpy, p).is_err());
}

#[test]
fn every_reduced_closure_dispatches_bit_identically() {
    for eos in closures() {
        match &eos {
            HemEos::StiffenedGas(g) => same(&eos, g),
            HemEos::IsentropicStiffenedGas(g) => same(&eos, g),
            HemEos::Barotropic(g) => same(&eos, g),
            HemEos::Linear(g) => same(&eos, g),
            other => panic!("not a reduced closure: {other:?}"),
        }
    }
}

#[test]
fn a_shock_tube_through_the_enum_matches_the_concrete_closure() {
    fn run<E: HemEquationOfState>(eos: E) -> Vec<(f64, f64)> {
        let geo = HemPipeGeometry::new(
            Length::new::<meter>(10.0),
            Area::new::<square_meter>(1.0),
            100,
        )
        .unwrap();
        let state = |p: f64| PrimitiveState::PressureTemperature {
            pressure: Pressure::new::<megapascal>(p),
            temperature: ThermodynamicTemperature::new::<kelvin>(400.0),
            velocity: Velocity::new::<meter_per_second>(0.0),
        };
        let mut pipe = HemPipeOf::new(
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
        pipe.run_until(Time::new::<second>(1.0e-3)).unwrap();
        pipe.snapshot()
            .cells
            .iter()
            .map(|c| {
                (
                    c.pressure.get::<megapascal>(),
                    c.density.get::<kilogram_per_cubic_meter>(),
                )
            })
            .collect()
    }
    let sg = StiffenedGasHem::water_liquid_le_metayer_2004();
    assert_eq!(run(HemEos::StiffenedGas(sg)), run(sg));
    // A use of the J/kg unit keeps the import honest for readers.
    assert!(AvailableEnergy::new::<joule_per_kilogram>(1.0).value > 0.0);
}
