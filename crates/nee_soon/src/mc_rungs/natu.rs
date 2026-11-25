//! **The `natu` rung's model**: a long bar of natural uranium metal, the
//! first demo of Neutronics Lesson 1, *Meet the Neutron* (gh:#906, #911;
//! moved 2026-11-25, gh:#1742, from `dhoby-ghaut/examples/monte_carlo_web/
//! natu/mod.rs`).
//!
//! The uranium is the natural-uranium reflector of ICSBEP IEU-MET-FAST-002
//! (Jemima), [`outram_mc_libs::vv::jemima`]'s `MATERIALS[MAT_REFLECTOR]`:
//! U-234, U-235 (0.72 % of the atoms) and U-238 at the densities the
//! recorded Jemima runs use, and the same three tapes. The bar is an
//! idealised shape (60 × 20 × 20 cm, vacuum outside), not an experiment; a
//! fresh neutron is born at one point near its left end.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::CsgModel;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::vv::jemima;

/// The case: the type the `natu` rung names.
pub struct Natu;

/// Half-length of the bar along x, cm.
pub const HALF_X: f64 = 30.0;
/// Half-width of the bar along y and z, cm.
pub const HALF_YZ: f64 = 10.0;
/// Where a fresh neutron is born: near the left end, on the axis, cm.
pub const BIRTH_X: f64 = -20.0;

/// Jemima's three tapes, `(label shown while loading, file)`.
pub const JOBS: [(&str, &str); 3] = [
    ("U-234", jemima::NUCLIDES[0].1),
    ("U-235", jemima::NUCLIDES[1].1),
    ("U-238", jemima::NUCLIDES[2].1),
];
const NUCLIDES: [(&str, &str, &str); 3] = [
    ("U-234", jemima::NUCLIDES[0].0, jemima::NUCLIDES[0].1),
    ("U-235", jemima::NUCLIDES[1].0, jemima::NUCLIDES[1].1),
    ("U-238", jemima::NUCLIDES[2].0, jemima::NUCLIDES[2].1),
];

/// The natural uranium: Jemima's reflector material, its nuclides indexed in
/// [`NUCLIDES`] order (the same order as `jemima::NUCLIDES`).
pub fn material() -> Material {
    let (name, comps) = jemima::MATERIALS[jemima::MAT_REFLECTOR];
    Material {
        id: 1,
        name: name.into(),
        temperature: jemima::TEMPERATURE_K,
        components: comps
            .iter()
            .map(|(n, d)| NuclideComponent {
                nuclide_idx: jemima::NUCLIDES.iter().position(|(m, _)| m == n).expect("Jemima's reflector names its own nuclides"),
                atom_density: *d,
            })
            .collect(),
    }
}

/// One box cell, six vacuum planes.
pub fn geometry() -> Geometry {
    let bc = BoundaryType::Vacuum;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -HALF_X, bc }),
        SurfaceKind::XPlane(XPlane { x0: HALF_X, bc }),
        SurfaceKind::YPlane(YPlane { y0: -HALF_YZ, bc }),
        SurfaceKind::YPlane(YPlane { y0: HALF_YZ, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: -HALF_YZ, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF_YZ, bc }),
    ];
    let hs = |surface_idx: usize, sense: HalfSpaceSense| RegionToken::HalfSpace { surface_idx, sense };
    let and = RegionToken::Intersection;
    let region = vec![
        hs(0, HalfSpaceSense::Outside),
        hs(1, HalfSpaceSense::Inside),
        and,
        hs(2, HalfSpaceSense::Outside),
        and,
        hs(3, HalfSpaceSense::Inside),
        and,
        hs(4, HalfSpaceSense::Outside),
        and,
        hs(5, HalfSpaceSense::Inside),
        and,
    ];
    let cells = vec![Cell::material(1, region, 0, jemima::TEMPERATURE_K)];
    Geometry { surfaces, cells, universes: vec![Universe { id: 0, cell_indices: vec![0] }], lattices: vec![], root_universe: 0 }
}

impl CsgModel for Natu {
    const TEMPERATURE_K: f64 = jemima::TEMPERATURE_K;
    const N_U235: usize = 1;
    /// A fresh neutron comes from a fission induced at 1 MeV, as in Godiva.
    const FRESH_INCIDENT_EV: f64 = 1.0e6;
    /// A fast neutron in uranium makes tens of collisions; one slowed into
    /// U-238's resonances may make a few hundred.
    const MAX_STATES: usize = 20_000;
    fn nuclides() -> &'static [(&'static str, &'static str, &'static str)] {
        &NUCLIDES
    }
    fn materials() -> Vec<Material> {
        vec![material()]
    }
    fn geometry() -> Geometry {
        geometry()
    }
    fn source_box() -> SourceBox {
        SourceBox { lower: Position::new(-HALF_X, -HALF_YZ, -HALF_YZ), upper: Position::new(HALF_X, HALF_YZ, HALF_YZ) }
    }
    fn fresh_birth(_rng: &mut u64) -> Position {
        Position::new(BIRTH_X, 0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bar holds Jemima's reflector uranium, three nuclides, one cell.
    #[test]
    fn the_bar_is_natural_uranium() {
        let m = Natu::materials();
        assert_eq!((m.len(), m[0].components.len()), (1, 3));
        assert_eq!(Natu::geometry().cells.len(), 1);
        assert_eq!(Natu::fresh_birth(&mut 0).x, BIRTH_X);
        assert_eq!(Natu::source_box().upper.x, HALF_X);
        assert_eq!(Natu::nuclides()[1].0, "U-235");
    }
}
