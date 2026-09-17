//! Stage 2 — mesh + Monte Carlo model (OpenMC / `outram-mc-libs`).
//!
//! Builds the Mk1 PB-FHR Monte Carlo reference model — annular pebble-bed
//! geometry (center reflector, active fuel region, blanket-pebble ring, outer
//! reflector, core barrel/downcomer/vessel; Tables 4.1/4.4/4.8–4.9 + Appendix C),
//! an FCC pebble lattice (packing 60 %, 3 cm pebbles, 4730 TRISO/pebble) — and
//! sets up `RegularMesh` + energy/spatial tallies that produce the flux weighting
//! for the 8-group MGXS (Eq. 2.23) and the per-burnup power fraction (Fig. 4.7).
//!
//! **Placeholder stage.** `outram-mc-libs` is data-free and pulls cross sections
//! from `njoy-outram-park-fork`. The enabling capabilities — multigroup mode +
//! `MGXSLibrary` (bead **op-6tz.15**) and `RegularMesh`/`MeshFilter` tallies
//! (bead **op-6tz.13**) — are still being built, so this stage composes their
//! public API only. Tracked by bead **op-fr2.2.3**.

use uom::si::f64::Length;
use uom::si::length::centimeter;

use super::{WorkflowError, WorkflowStage};

/// Bead tracking the mesh + Monte Carlo stage.
pub const STAGE_BEAD: &str = "op-fr2.2.3";

/// Key radial dimensions of the Mk1 annular core (Table 4.1 / Appendix C),
/// used to lay out the CSG/mesh geometry. Radii are outer radii from the axis.
#[derive(Debug, Clone, Copy)]
pub struct Mk1CoreGeometry {
    /// Center (inner) graphite reflector outer radius (35 cm, Table 4.1).
    pub center_reflector_radius: Length,
    /// Active fuel-region outer radius (105 cm, Appendix C Table C.2).
    pub fuel_region_outer_radius: Length,
    /// Blanket-pebble ring outer radius (125 cm, Appendix C Table C.3).
    pub blanket_outer_radius: Length,
    /// Outer graphite reflector outer radius (165 cm, Appendix C Table C.4).
    pub outer_reflector_radius: Length,
    /// Reactor vessel outer radius (175 cm, Table 4.9).
    pub vessel_outer_radius: Length,
    /// Fuel-pebble outer diameter (3 cm, Table 4.3).
    pub pebble_diameter: Length,
}

impl Mk1CoreGeometry {
    /// The reference Mk1 geometry (no outer shield; Table 4.9 / Appendix C).
    pub fn reference() -> Self {
        Self {
            center_reflector_radius: Length::new::<centimeter>(35.0),
            fuel_region_outer_radius: Length::new::<centimeter>(105.0),
            blanket_outer_radius: Length::new::<centimeter>(125.0),
            outer_reflector_radius: Length::new::<centimeter>(165.0),
            vessel_outer_radius: Length::new::<centimeter>(175.0),
            pebble_diameter: Length::new::<centimeter>(3.0),
        }
    }
}

/// Stage-2 driver: builds the Mk1 Monte Carlo model + MGXS/power tallies.
#[derive(Debug, Clone)]
pub struct MeshMonteCarloStage {
    geometry: Mk1CoreGeometry,
    /// FCC pebble packing fraction (0.60, Table 4.1).
    pebble_packing_fraction: f64,
    /// TRISO particles per fuel pebble (4730, Table 4.1).
    triso_per_pebble: usize,
}

impl Default for MeshMonteCarloStage {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshMonteCarloStage {
    /// Creates the stage with the reference Mk1 geometry and packing.
    pub fn new() -> Self {
        Self {
            geometry: Mk1CoreGeometry::reference(),
            pebble_packing_fraction: 0.60,
            triso_per_pebble: 4730,
        }
    }

    /// The Mk1 core geometry this stage meshes.
    pub fn geometry(&self) -> Mk1CoreGeometry {
        self.geometry
    }

    /// FCC pebble packing fraction (0.60).
    pub fn pebble_packing_fraction(&self) -> f64 {
        self.pebble_packing_fraction
    }

    /// TRISO particles per fuel pebble (4730).
    pub fn triso_per_pebble(&self) -> usize {
        self.triso_per_pebble
    }

    /// Build the Monte Carlo model, run k-eigenvalue, and tally the 8-group MGXS
    /// + per-burnup power fractions.
    ///
    /// # Placeholder
    ///
    /// Returns [`WorkflowError::NotYetImplemented`]. The intended body composes
    /// `outram-mc-libs`' CSG/mesh geometry, k-eigenvalue solve, and
    /// `RegularMesh`/`MeshFilter` energy-binned tallies (blocked on beads
    /// op-6tz.15 / op-6tz.13) fed by `njoy-outram-park-fork` cross sections.
    // TODO(op-fr2.2.3): build Mk1 CSG geometry + FCC lattice, run k-eigenvalue,
    // add RegularMesh energy/spatial tallies -> 8-group MGXS + Fig 4.7 power
    // fractions (blocked on op-6tz.15, op-6tz.13). Compose outram-mc API only.
    pub fn run(&self) -> Result<(), WorkflowError> {
        Err(WorkflowError::NotYetImplemented {
            stage: WorkflowStage::MeshAndMonteCarlo,
            bead: STAGE_BEAD,
        })
    }
}
