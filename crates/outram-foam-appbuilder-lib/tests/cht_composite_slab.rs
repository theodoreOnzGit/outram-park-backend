//! V&V V1 of gh:#1156: the coupled-temperature interface
//! (`compressible::turbulentTemperatureCoupledBaffleMixed`, ported in
//! `solvers::cht_multi_region_foam`) against the analytic composite slab.
//!
//! **Methodology.** Two regions on separate 1-D meshes: A on x ∈ [0, 10 mm]
//! (k = 0.5 W/(m K), 10 cells), B on x ∈ [10, 40 mm] (k = 16 W/(m K), 15
//! cells), T = 400 K at x = 0 and 300 K at x = 40 mm, steady, iterated by the
//! region loop to a pass change of 1e-11 K. Three variants: two solids; two
//! solids with a contact layer (0.5 mm at k = 0.1, h_c = 200 W/(m² K)); A a
//! **fluid at rest** (`PimpleFoam`, `frozenFlow`, k = 0.6, ρ c_p = 4e6) and B
//! a solid. Reference: series resistance, `q = ΔT/(L_A/k_A + 1/h_c +
//! L_B/k_B)`, T piecewise linear. Pass: max |T − T_exact| < 1e-8 K and both
//! sides' interface heat within 1e-10 of q (the predictions in
//! `verification_and_validation/cht_double_pipe_gh1156.md`, written before
//! the run).
//!
//! **Results (2026-11-03):** two solids 46 passes, max |T − T_exact| 5.8e-9 K,
//! interface heat −6.3e-11 relative on both sides; contact layer 18 passes,
//! 7.8e-9 K, +7.4e-11; fluid at rest + solid 51 passes, 7.7e-9 K, −8.5e-11.
//! The T error is the 1e-12 solve tolerance's (at 1e-15 it is 1.2e-11 K and
//! the solid pair takes 57 passes, against 58 predicted). Interpretation: the
//! ported condition is exact for the piecewise-linear solution. Full table:
//! the V&V record, V1.
use std::sync::Arc;

use outram_foam_appbuilder_lib::io::control_dict::ControlDict;
use outram_foam_appbuilder_lib::io::fv_schemes::{DdtScheme, FvSchemes};
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::cht_multi_region_foam::{line_mesh, ChtMultiRegionFoam, CoupledTemperature, RegionId, SolidRegion};
use outram_foam_appbuilder_lib::solvers::energy::{EnergyEquation, IncompressibleThermo};
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::*;
use uom::si::f64::{Length, TemperatureInterval, ThermalConductivity};
use uom::si::temperature_interval::kelvin;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

const LA: f64 = 0.01;
const LB: f64 = 0.03;
const KA: f64 = 0.5;
const KB: f64 = 16.0;

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Solids,
    Layer,
    FluidSolid,
}

fn build(v: Variant) -> ChtMultiRegionFoam {
    let len = Length::new::<meter>;
    let ma = Arc::new(line_mesh(10, len(LA), len(0.0), ["left", "iface"]));
    let mb = Arc::new(line_mesh(15, len(LB), len(LA), ["iface", "right"]));
    let mut b = SolidRegion::new("b", mb.clone(), IncompressibleThermo::si(8000.0, 500.0, KB), 350.0);
    b.energy.ddt = DdtScheme::SteadyState;
    b.energy.set_patch("right", BoundaryCondition::FixedValue(300.0));
    let (ka, rid) = if v == Variant::FluidSolid { (0.6, RegionId::Fluid(0)) } else { (KA, RegionId::Solid(0)) };
    let mut ea = EnergyEquation::uniform(ma.clone(), IncompressibleThermo::si(1000.0, 4000.0, ka), 350.0);
    ea.ddt = DdtScheme::SteadyState;
    ea.set_patch("left", BoundaryCondition::FixedValue(400.0));
    let mut c = CoupledTemperature::new(rid, ma.clone(), 1, RegionId::Solid(if v == Variant::FluidSolid { 0 } else { 1 }), mb, 0);
    if v == Variant::Layer {
        c = c.with_layers(&[(Length::new::<meter>(5e-4), ThermalConductivity::new::<watt_per_meter_kelvin>(0.1))]);
    }
    let mut s = if v == Variant::FluidSolid {
        let mut sol = FvSolution::default();
        sol.pimple.frozen_flow = true;
        let mut f = PimpleFoam::new(ma, ControlDict::default(), FvSchemes::default(), sol);
        f.energy = Some(ea);
        ChtMultiRegionFoam::new(vec![f], vec![b], vec![c]).unwrap()
    } else {
        let mut a = SolidRegion::new("a", ma, IncompressibleThermo::si(1000.0, 4000.0, KA), 350.0);
        a.energy = ea;
        ChtMultiRegionFoam::new(vec![], vec![a, b], vec![c]).unwrap()
    };
    s.energy_outer_correctors = 5000;
    s.energy_outer_tolerance = TemperatureInterval::new::<kelvin>(1e-11);
    s
}

#[test]
fn composite_slab_matches_series_resistance() {
    println!("variant      passes  max|T-T_exact| [K]  q_A/q-1      q_B/q-1      T_i [K]");
    for (v, name) in [(Variant::Solids, "solids"), (Variant::Layer, "layer"), (Variant::FluidSolid, "fluid-solid")] {
        let mut s = build(v);
        let r = s.step().unwrap();
        let ka = if v == Variant::FluidSolid { 0.6 } else { KA };
        let rc = if v == Variant::Layer { 5e-4 / 0.1 } else { 0.0 };
        let q = 100.0 / (LA / ka + rc + LB / KB);
        // Exact piecewise-linear profile; the layer sits at x = L_A.
        let ta_face = 400.0 - q * LA / ka;
        let tb_face = 300.0 + q * LB / KB;
        let exact = |x: f64| if x < LA { 400.0 - q * x / ka } else { tb_face - q * (x - LA) / KB };
        let ta = match v {
            Variant::FluidSolid => &s.fluids[0].energy.as_ref().unwrap().t,
            _ => &s.solids[0].energy.t,
        };
        let tb = &s.solids[if v == Variant::FluidSolid { 0 } else { 1 }].energy.t;
        let mut err = 0.0_f64;
        for t in [ta, tb] {
            for c in 0..t.mesh.n_cells {
                err = err.max((t.internal[c] - exact(t.mesh.cell_centres[c].x)).abs());
            }
        }
        let qa = -s.interface_heat_in(s.interfaces[0].a).get::<watt>();
        let qb = s.interface_heat_in(s.interfaces[0].b).get::<watt>();
        println!(
            "{name:<12} {:>6}  {err:>18.3e}  {:>+11.3e}  {:>+11.3e}  {:.9} / {:.9} (exact {ta_face:.9} / {tb_face:.9})",
            r.passes,
            qa / q - 1.0,
            qb / q - 1.0,
            ta.boundary[1].values[0],
            tb.boundary[0].values[0]
        );
        assert!(r.last_pass_change <= 1e-11, "{name}: not converged in {} passes", r.passes);
        assert!(err < 1e-8, "{name}: max |T - T_exact| {err}");
        assert!((qa / q - 1.0).abs() < 1e-10 && (qb / q - 1.0).abs() < 1e-10, "{name}: {qa} {qb} {q}");
    }
}
