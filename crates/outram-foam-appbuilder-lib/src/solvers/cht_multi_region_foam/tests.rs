use super::*;
use crate::io::control_dict::ControlDict;
use crate::io::fv_schemes::{DdtScheme, FvSchemes};
use crate::io::fv_solution::FvSolution;

/// Metres as a `Length`.
fn metres(v: f64) -> Length {
    Length::new::<meter>(v)
}

/// Two solid slabs, A on [0, 1 cm] (k = 1) and B on [1, 3 cm] (k = 4),
/// 400 K / 300 K at the outer ends, steady.
fn two_slabs() -> ChtMultiRegionFoam {
    let ma = Arc::new(line_mesh(5, metres(0.01), metres(0.0), ["left", "iface"]));
    let mb = Arc::new(line_mesh(8, metres(0.02), metres(0.01), ["iface", "right"]));
    let mut a = SolidRegion::new("a", ma.clone(), IncompressibleThermo::si(1.0, 1.0, 1.0), 350.0);
    let mut b = SolidRegion::new("b", mb.clone(), IncompressibleThermo::si(1.0, 1.0, 4.0), 350.0);
    a.energy.ddt = DdtScheme::SteadyState;
    b.energy.ddt = DdtScheme::SteadyState;
    a.energy.set_patch("left", BoundaryCondition::FixedValue(400.0));
    b.energy.set_patch("right", BoundaryCondition::FixedValue(300.0));
    let c = CoupledTemperature::new(RegionId::Solid(0), ma, 1, RegionId::Solid(1), mb, 0);
    let mut s = ChtMultiRegionFoam::new(vec![], vec![a, b], vec![c]).unwrap();
    s.energy_outer_correctors = 500;
    s.energy_outer_tolerance = TemperatureInterval::new::<kelvin_interval>(1e-11);
    s
}

/// The default couples the interface (gh:#1156 "physics on by default"):
/// built through the ordinary path, the slab carries the series-resistance
/// heat flux, and the ablation carries none.
#[test]
fn interface_coupling_is_on_by_default() {
    let mut s = two_slabs();
    assert_eq!(s.interfaces[0].mode, InterfaceMode::Coupled);
    let r = s.step().unwrap();
    assert!(r.passes < 500 && r.last_pass_change <= 1e-11, "{r:?}");
    // q = ΔT / (L_A/k_A + L_B/k_B) = 100 / (0.01 + 0.005) W/m².
    let want = 100.0 / 0.015;
    let qa = s.interface_heat_in(s.interfaces[0].a).get::<watt>();
    let qb = s.interface_heat_in(s.interfaces[0].b).get::<watt>();
    assert!((qb / want - 1.0).abs() < 1e-9 && (qa / -want - 1.0).abs() < 1e-9, "qa {qa} qb {qb} want {want}");

    let mut off = two_slabs().without_interface_coupling();
    assert_eq!(off.interfaces[0].mode, InterfaceMode::Adiabatic);
    off.step().unwrap();
    assert_eq!(off.interface_heat_in(off.interfaces[0].a).get::<watt>(), 0.0);
    // Adiabatic at the interface: each slab takes its fixed end's value.
    assert!(off.solids[0].energy.t.internal.as_slice().iter().all(|t| (t - 400.0).abs() < 1e-8));
}

#[test]
fn layers_set_the_contact_conductance_and_pairing_is_checked() {
    let s = two_slabs();
    let c = s.interfaces[0].clone();
    assert_eq!(c.max_pairing_gap(), 0.0);
    let l = c.clone().with_layers(&[
        (Length::new::<meter>(1e-3), ThermalConductivity::new::<watt_per_meter_kelvin>(0.5)),
        (Length::new::<meter>(1e-3), ThermalConductivity::new::<watt_per_meter_kelvin>(1.0)),
    ]);
    match l.mode {
        InterfaceMode::CoupledWithLayers { contact_conductance } => assert!((contact_conductance - 1.0 / 3e-3).abs() < 1e-9),
        m => panic!("{m:?}"),
    }
    // No layers given: still the plain coupling.
    assert_eq!(c.clone().with_layers(&[]).mode, InterfaceMode::Coupled);
    // A pairing onto a patch of a different size is not one-to-one.
    let mut bad = c;
    bad.pairing.patch_b = 2; // "sides", 32 faces
    assert_eq!(bad.max_pairing_gap(), f64::INFINITY);
}

#[test]
fn configuration_errors_are_reported() {
    let m = Arc::new(line_mesh(3, metres(0.01), metres(0.0), ["left", "right"]));
    let f = PimpleFoam::new(m.clone(), ControlDict::default(), FvSchemes::default(), FvSolution::default());
    let e = ChtMultiRegionFoam::new(vec![f], vec![], vec![]).unwrap_err();
    assert!(format!("{e}").contains("no energy equation"), "{e}");
    let c = CoupledTemperature::new(RegionId::Solid(0), m.clone(), 0, RegionId::Solid(3), m, 1);
    let e = ChtMultiRegionFoam::new(vec![], vec![], vec![c]).unwrap_err();
    assert!(format!("{e}").contains("missing region"), "{e}");
}

/// A fluid region (PimpleFoam, frozen, at rest) coupled to a solid: the
/// fluid's α-form equation and the solid's agree on the interface flux.
#[test]
fn fluid_at_rest_couples_to_a_solid() {
    let mf = Arc::new(line_mesh(6, metres(0.006), metres(0.0), ["left", "iface"]));
    let ms = Arc::new(line_mesh(4, metres(0.004), metres(0.006), ["iface", "right"]));
    let mut sol = FvSolution::default();
    sol.pimple.frozen_flow = true;
    let mut f = PimpleFoam::new(mf.clone(), ControlDict::default(), FvSchemes::default(), sol);
    let mut e = EnergyEquation::uniform(mf.clone(), IncompressibleThermo::si(1000.0, 4000.0, 0.6), 300.0);
    e.ddt = DdtScheme::SteadyState;
    e.set_patch("left", BoundaryCondition::FixedValue(360.0));
    f.energy = Some(e);
    let mut s = SolidRegion::new("wall", ms.clone(), IncompressibleThermo::si(8000.0, 500.0, 15.0), 300.0);
    s.energy.ddt = DdtScheme::SteadyState;
    s.energy.set_patch("right", BoundaryCondition::FixedValue(300.0));
    let c = CoupledTemperature::new(RegionId::Fluid(0), mf, 1, RegionId::Solid(0), ms, 0);
    let mut cht = ChtMultiRegionFoam::new(vec![f], vec![s], vec![c]).unwrap();
    cht.energy_outer_correctors = 2000;
    cht.energy_outer_tolerance = TemperatureInterval::new::<kelvin_interval>(1e-11);
    let r = cht.step().unwrap();
    assert!(r.last_pass_change <= 1e-11, "{r:?}");
    let want = 60.0 / (0.006 / 0.6 + 0.004 / 15.0);
    let qs = cht.interface_heat_in(cht.interfaces[0].b).get::<watt>();
    let qf = cht.interface_heat_in(cht.interfaces[0].a).get::<watt>();
    assert!((qs / want - 1.0).abs() < 1e-9 && (qf / -want - 1.0).abs() < 1e-9, "{qs} {qf} {want}");
}

/// gh:#1531: `refGrad = (qr + qrNbr)/kappa` with `qrNbr` paired through the face map; `valueFraction`
/// and `refValue` are those of the flux-free condition, and `(None, None)` leaves `refGrad` exactly 0.
#[test]
fn radiative_flux_enters_ref_grad_only() {
    let s = two_slabs();
    let c = &s.interfaces[0];
    let (ta, tb) = (&s.solids[0].energy.t, &s.solids[1].energy.t);
    let k = |v: f64| ThermalConductivity::new::<watt_per_meter_kelvin>(v);
    let mode = fvm::DeltaCoeff::default();
    let plain = c.update_coeffs(true, ta, k(1.0), mode, tb, k(4.0), mode);
    let with = c.update_coeffs_with_flux(true, ta, k(1.0), mode, tb, k(4.0), mode, Some(&[-8.0]), Some(&[-2.0]));
    let none = c.update_coeffs_with_flux(true, ta, k(1.0), mode, tb, k(4.0), mode, None, None);
    match (plain, with, none) {
        (
            BoundaryCondition::MixedField { value_fraction: w0, ref_value: r0, ref_grad: g0 },
            BoundaryCondition::MixedField { value_fraction: w1, ref_value: r1, ref_grad: g1 },
            BoundaryCondition::MixedField { ref_grad: g2, .. },
        ) => {
            assert_eq!(w0.as_slice(), w1.as_slice());
            assert_eq!(r0.as_slice(), r1.as_slice());
            assert_eq!(g0.as_slice(), &[0.0]);
            assert_eq!(g2.as_slice(), &[0.0]);
            assert_eq!(g1.as_slice(), &[(-8.0 + -2.0) / 1.0]);
        }
        _ => panic!("expected MixedField"),
    }
}
