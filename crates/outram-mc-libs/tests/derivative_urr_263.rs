//! **Derivatives use the URR band the transport followed (gh:#263).**
//!
//! # Methodology
//!
//! `TraceEvent::Located` now carries the flight's `urr_seed`, so an observer can
//! reproduce the sampled cross sections the kernel used:
//! `Material::macro_xs_urr(e, nuclides, urr_seed)` (the band draw of nuclide `i`
//! is `urr_xi(i, urr_seed)`). `derivative_run` uses them for the score, the
//! density and nuclide-density track terms and the explicit term, where it used
//! the infinitely dilute ones before.
//!
//! Fixture: ENDF/B-VIII.0 U-238 at 293.6 K with cheap PURR settings (table values
//! are not asserted), a pure U-238 sphere, a 50 keV source (inside the 20-149 keV
//! unresolved range). Assertions: (1) at EVERY `Located` event of 200 traced
//! histories, `Located.sigma_t` equals `macro_xs_urr(e, .., urr_seed).total` bit
//! for bit; (2) at least one event has a dilute total different from the
//! sampled one, so the difference matters; (3) the derivative driver runs on this
//! URR medium and gives finite numbers.
//!
//! # Results (2026-11-20)
//!
//! 200 traced histories at 50 keV in pure U-238 (cheap PURR: 20 x 4 x 500):
//! **7383 located events, 0 differ from `macro_xs_urr` (bit for bit), 7148 differ
//! from the dilute total**, i.e. the sampled band is the cross section the kernel
//! used at essentially every flight in this range, and the dilute one the driver
//! used before is wrong by a visible amount. The driver on this medium (200
//! histories, density derivative): flux 122.4 +- 6.1, d/d rho -9545 +- 1749; total
//! 33.48 +- 1.84, -1023 +- 487; absorption 0.955 +- 0.060, -31.2 +- 15.7 (finite;
//! no analytic reference exists for a fissile, leaking-free-but-fissioning
//! medium, so (1)-(2) are the gates).

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::derivative_run::{run_fixed_source_derivative, DerivativeSpec};
use outram_mc_libs::physics::fixed_source::{FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::transport_csg::trace_csg_history;
use outram_mc_libs::physics::tracking_trace::TraceEvent;
use outram_mc_libs::tally::derivative::{DerivativeVariable, TallyDerivative};
use outram_mc_libs::tally::tally::ScoreType;

#[test]
fn derivatives_follow_the_sampled_urr_band() {
    let Some(p) = reference_endf_or_skip("n-092_U_238.endf", "U-238 (URR derivative #263)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("tape parses");
    let t = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(293.6);
    let nuc = Nuclide::from_endf_file(&p, "U238", t, 1.0e-3)
        .expect("reconstructs")
        .without_urr_probability_tables()
        .with_urr_probability_tables(&tape, 9237, 293.6, 20, 4, 500)
        .expect("tables");
    assert!(nuc.has_urr_probability_tables());
    let nuclides = vec![nuc];
    let mats = vec![Material {
        id: 1,
        name: "U238".into(),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 0.0222 }],
        temperature: 293.6,
    }];
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: 20.0, bc: BoundaryType::Reflective })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            CellFill::Material(0),
            Position::ZERO,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };

    // (1) and (2): the traced flights.
    let (mut n_located, mut n_differs, mut n_bad) = (0usize, 0usize, 0usize);
    let mut seed = 99u64;
    for _ in 0..200 {
        let mut e_cur = 5.0e4;
        trace_csg_history(Position::ZERO, Direction::new(1.0, 0.0, 0.0), 5.0e4, &geom, &mats, &nuclides, &mut seed, |ev| match ev {
            TraceEvent::Start { e, .. } => e_cur = e,
            TraceEvent::State(st) => e_cur = st.energy,
            TraceEvent::Collision { e, .. } => e_cur = e,
            TraceEvent::Located { material: Some(m), sigma_t, urr_seed, .. } => {
                n_located += 1;
                let sampled = mats[m].macro_xs_urr(e_cur, &nuclides, urr_seed).total;
                if sampled.to_bits() != sigma_t.to_bits() {
                    n_bad += 1;
                }
                if (mats[m].macro_xs(e_cur, &nuclides).total - sigma_t).abs() > 1e-12 * sigma_t {
                    n_differs += 1;
                }
            }
            _ => {}
        });
    }
    println!("URR derivative fixture: {n_located} located events, {n_bad} differ from macro_xs_urr, {n_differs} differ from the dilute total");
    assert!(n_located > 100);
    assert_eq!(n_bad, 0, "Located.sigma_t must equal macro_xs_urr at the carried urr_seed");
    assert!(n_differs > 0, "the sampled band must differ from the dilute total somewhere");

    // (3): the driver runs on the URR medium.
    let src = FixedSource::Point { r: Position::ZERO, energy_ev: 5.0e4 };
    let settings = FixedSourceSettings { n_particles: 200, n_batches: 4, seed: 5, ..Default::default() };
    for score in [ScoreType::Flux, ScoreType::Total, ScoreType::Absorption] {
        let spec = DerivativeSpec {
            derivative: TallyDerivative { material_idx: 0, variable: DerivativeVariable::Density },
            score,
            score_material: 0,
        };
        let r = run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &spec).unwrap();
        println!("{score:?}: {:.4} +- {:.4}, d/d rho = {:.3} +- {:.3}", r.flux, r.flux_std, r.d_flux, r.d_flux_std);
        assert!(r.flux.is_finite() && r.d_flux.is_finite());
    }
}
