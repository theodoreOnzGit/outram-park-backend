//! Stage 1 — multigroup cross-section (MGXS) generation via `njoy`.
//!
//! Produces the cell-homogenised 8-group macroscopic cross sections the
//! deterministic SP3 model consumes, from ENDF/B-VII.0, following Wang Eq. 2.23
//! (flux-weighted tally) and the feedback parametrisation Eqs. 2.24–2.26
//! (linear-in-density for flibe, linear-in-log-T for fuel Doppler).
//!
//! **Placeholder stage.** `njoy-outram-park-fork` owns all nuclear-data code and
//! does not yet expose a public MGXS export entry point for deterministic
//! consumers — that API is requested in bead **op-cjw.24**. This stage scaffolds
//! the *call* into that future API; it does not (and must not) implement MGXS
//! inside `njoy` from here. Tracked by bead **op-fr2.2.2**.

use uom::si::f64::Energy;

use super::case;
use super::{WorkflowError, WorkflowStage};

/// Bead tracking the MGXS-generation stage.
pub const STAGE_BEAD: &str = "op-fr2.2.2";

/// Bead requesting the required MGXS-export API from `njoy-outram-park-fork`.
pub const NJOY_API_BEAD: &str = "op-cjw.24";

/// The set of homogenised material regions the Mk1 SP3 model needs cross
/// sections for (Table 4.4). One MGXS set is generated per region, each
/// parametrised for feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mk1MaterialRegion {
    /// Fuel-pebble region (graphite shell + fuel annulus + graphite core + flibe).
    FuelPebble,
    /// Graphite blanket-pebble region + flibe.
    BlanketPebble,
    /// Center graphite reflector.
    CenterReflector,
    /// Outer graphite (borated) reflector.
    OuterReflector,
    /// Boron-carbide control rod.
    ControlRod,
    /// Structural stainless steel (SS316 core barrel / vessel).
    StainlessSteel,
    /// Flibe coolant channels.
    Flibe,
}

impl Mk1MaterialRegion {
    /// All regions that require an MGXS set for the Mk1 core model.
    pub fn all() -> [Mk1MaterialRegion; 7] {
        [
            Self::FuelPebble,
            Self::BlanketPebble,
            Self::CenterReflector,
            Self::OuterReflector,
            Self::ControlRod,
            Self::StainlessSteel,
            Self::Flibe,
        ]
    }
}

/// Stage-1 driver: generates 8-group MGXS for every [`Mk1MaterialRegion`].
#[derive(Debug, Clone)]
pub struct MgxsGenerationStage {
    group_lower_bounds: [Energy; case::NUM_ENERGY_GROUPS],
}

impl Default for MgxsGenerationStage {
    fn default() -> Self {
        Self::new()
    }
}

impl MgxsGenerationStage {
    /// Creates the stage with the Mk1 8-group structure (Table 3.4).
    pub fn new() -> Self {
        Self {
            group_lower_bounds: case::energy_group_lower_bounds(),
        }
    }

    /// The 8-group lower energy boundaries this stage generates constants on.
    pub fn group_lower_bounds(&self) -> [Energy; case::NUM_ENERGY_GROUPS] {
        self.group_lower_bounds
    }

    /// Generate the 8-group MGXS for all Mk1 material regions.
    ///
    /// # Placeholder
    ///
    /// Returns [`WorkflowError::NotYetImplemented`]. The intended body composes
    /// `njoy-outram-park-fork`'s (future) MGXS-export API — see bead
    /// [`NJOY_API_BEAD`] — with the flux weighting produced by the Monte Carlo
    /// stage (Eq. 2.23), then fits the density / log-T feedback coefficients
    /// (Eqs. 2.25–2.26). Do not implement MGXS inside `njoy` from here.
    // TODO(op-fr2.2.2): wire njoy MGXS export (blocked on op-cjw.24) + MC flux
    // weighting from Stage 2, then fit feedback coefficients.
    pub fn run(&self) -> Result<(), WorkflowError> {
        Err(WorkflowError::NotYetImplemented {
            stage: WorkflowStage::MgxsGeneration,
            bead: STAGE_BEAD,
        })
    }
}
