//! **The Jemima rung's model**: ICSBEP IEU-MET-FAST-002, a cylinder of 16 %
//! enriched uranium metal inside natural-uranium reflectors (gh:#859; moved
//! 2026-11-25, gh:#1742, from `dhoby-ghaut/examples/monte_carlo_web/
//! jemima/mod.rs`).
//!
//! **Nothing is retyped here.** Geometry, materials and tapes are
//! [`outram_mc_libs::vv::jemima`], the model the recorded result (route 4 of
//! the five-route study) and the acceptance check were measured on.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::{uniform_in_cylinder, CsgModel};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::vv::jemima as model;

/// The case: the type the `jemima` rung names.
pub struct Jemima;

/// The three tapes, `(label shown while loading, file)`, in
/// [`model::NUCLIDES`] order.
pub const JOBS: [(&str, &str); 3] = [("U-234", model::NUCLIDES[0].1), ("U-235", model::NUCLIDES[1].1), ("U-238", model::NUCLIDES[2].1)];
const NUCLIDES: [(&str, &str, &str); 3] = [
    ("U-234", model::NUCLIDES[0].0, model::NUCLIDES[0].1),
    ("U-235", model::NUCLIDES[1].0, model::NUCLIDES[1].1),
    ("U-238", model::NUCLIDES[2].0, model::NUCLIDES[2].1),
];

impl CsgModel for Jemima {
    const TEMPERATURE_K: f64 = model::TEMPERATURE_K;
    const N_U235: usize = 1;
    /// A fresh chain starts from a 1 MeV-incident fission, as in Godiva.
    const FRESH_INCIDENT_EV: f64 = 1.0e6;
    /// A fast neutron here makes a few tens of collisions.
    const MAX_STATES: usize = 20_000;
    fn nuclides() -> &'static [(&'static str, &'static str, &'static str)] {
        &NUCLIDES
    }
    fn materials() -> Vec<Material> {
        model::materials()
    }
    fn geometry() -> Geometry {
        model::geometry()
    }
    fn source_box() -> SourceBox {
        model::source_box()
    }
    /// The whole assembly: the natural-uranium reflector fissions too.
    fn entropy_box() -> SourceBox {
        SourceBox { lower: Position::new(-model::R_OUT, -model::R_OUT, model::Z_BOT), upper: Position::new(model::R_OUT, model::R_OUT, model::Z_TOP) }
    }
    /// A uniform point of the core.
    fn fresh_birth(rng: &mut u64) -> Position {
        uniform_in_cylinder(rng, model::R_CORE, model::Z_CORE_LO, model::Z_CORE_HI)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The case names the benchmark's tapes and births inside its core.
    #[test]
    fn the_case_is_the_benchmark_model() {
        assert_eq!(Jemima::nuclides().len(), 3);
        assert_eq!(Jemima::materials().len(), model::materials().len());
        assert!(!Jemima::geometry().cells.is_empty());
        let mut rng = 1u64;
        let p = Jemima::fresh_birth(&mut rng);
        assert!(p.x * p.x + p.y * p.y < model::R_CORE * model::R_CORE);
        let e = Jemima::entropy_box();
        assert_eq!(e.upper.z, model::Z_TOP);
        assert_eq!(Jemima::source_box().lower.z, model::source_box().lower.z);
    }
}
