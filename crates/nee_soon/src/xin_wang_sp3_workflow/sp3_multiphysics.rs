//! Stage 3 — SP3 multiphysics (GeN-Foam, `outram-foam-appbuilder-lib`).
//!
//! Drives the GeN-Foam SP3 neutronics coupled to the porous-media TH model + the
//! multi-scale fuel-pebble/TRISO conduction feedback over the control-rod-removal
//! transient. The SP3 system is Wang Eq. 2.22 / D.12 (two moment fields
//! `Phi0 = phi0 + 2*phi2` and `phi2`; six delayed groups), fed with the Stage-1/2
//! MGXS, coupled through the `multi_region` outer loop with Ergun/Wakao TH
//! closures ($E_1=150$, $E_2=1.75$, $c_F=0.52$).
//!
//! **Placeholder stage, blocked on the in-progress GeN-Foam SP3 port.** It codes
//! against the intended interface in
//! [`outram_foam_appbuilder_lib::genfoam::neutronics`]:
//! `Sp3Neutronics::with_cross_sections(..)` → `solve_eigenvalue()` → `step(dt)`,
//! or wrapped as `NeutronicsModel::Sp3(..)` for the shared `power()`/`k_eff()`
//! surface. Two blockers remain:
//!
//! 1. the SP3 solver port itself (`Sp3Neutronics` eigenvalue/transient solvers +
//!    boundary handling / benchmark) — bead **op-p6p.15**; today
//!    `Sp3Neutronics::new` is a state-only scaffold whose solvers return
//!    `ModelNotImplemented(Sp3)`;
//! 2. mesh-based neutronics is **not yet a `RegionModel` variant** in
//!    `multi_region::outer_iteration`, so SP3 cannot be driven through
//!    `MultiPhysicsSolver` yet ("wired-in-waiting") — bead **op-p6p.8.4**.
//!
//! Tracked by bead **op-fr2.2.4**.

use outram_foam_appbuilder_lib::genfoam::neutronics::NeutronicsModelKind;

use super::case::{self, ControlRodRemovalTransient};
use super::{WorkflowError, WorkflowStage};

/// Bead tracking the SP3-multiphysics stage.
pub const STAGE_BEAD: &str = "op-fr2.2.4";

/// Bead for the in-progress GeN-Foam SP3 solver port (blocker).
pub const GENFOAM_SP3_PORT_BEAD: &str = "op-p6p.15";

/// Bead for the missing mesh-neutronics `RegionModel` coupling variant (blocker).
pub const GENFOAM_COUPLING_BEAD: &str = "op-p6p.8.4";

/// Ergun / Wakao porous-media closure values for the Mk1 pebble bed (Table 4.10).
#[derive(Debug, Clone, Copy)]
pub struct PorousMediaClosures {
    /// Ergun viscous coefficient `E_1` (150).
    pub ergun_e1: f64,
    /// Ergun inertial coefficient `E_2` (1.75).
    pub ergun_e2: f64,
    /// Non-dimensional Forchheimer drag coefficient `c_F` (0.52).
    pub forchheimer_cf: f64,
}

impl PorousMediaClosures {
    /// The Mk1 values from Table 4.10.
    pub fn mk1() -> Self {
        Self {
            ergun_e1: 150.0,
            ergun_e2: 1.75,
            forchheimer_cf: 0.52,
        }
    }
}

/// Stage-3 driver: SP3 neutronics coupled to porous-media TH for the transient.
#[derive(Debug, Clone)]
pub struct Sp3MultiphysicsStage {
    energy_groups: usize,
    delayed_groups: usize,
    closures: PorousMediaClosures,
    transient: ControlRodRemovalTransient,
}

impl Default for Sp3MultiphysicsStage {
    fn default() -> Self {
        Self::new()
    }
}

impl Sp3MultiphysicsStage {
    /// Creates the stage for the 8-group / 6-delayed-group Mk1 model and the
    /// Figure-4.29 control-rod-removal transient.
    pub fn new() -> Self {
        Self {
            energy_groups: case::NUM_ENERGY_GROUPS,
            delayed_groups: case::NUM_DELAYED_GROUPS,
            closures: PorousMediaClosures::mk1(),
            transient: ControlRodRemovalTransient::fig_4_29(),
        }
    }

    /// Number of energy groups the SP3 model runs (8).
    pub fn energy_groups(&self) -> usize {
        self.energy_groups
    }

    /// Number of delayed-neutron precursor groups (6).
    pub fn delayed_groups(&self) -> usize {
        self.delayed_groups
    }

    /// The porous-media TH closures used for the coupled run.
    pub fn porous_media_closures(&self) -> PorousMediaClosures {
        self.closures
    }

    /// The transient this stage drives.
    pub fn transient(&self) -> ControlRodRemovalTransient {
        self.transient
    }

    /// The GeN-Foam neutronics model kind this stage targets — SP3.
    ///
    /// This is a real compile-time dependency on the GeN-Foam port's
    /// [`NeutronicsModelKind`] enum, pinning the intended model even while the
    /// solver body is still being ported.
    pub fn target_neutronics_kind(&self) -> NeutronicsModelKind {
        NeutronicsModelKind::Sp3
    }

    /// Solve the SP3 eigenvalue steady state, then step the coupled SP3 + TH
    /// system through the control-rod-removal transient.
    ///
    /// # Placeholder
    ///
    /// Returns [`WorkflowError::NotYetImplemented`]. The intended body builds a
    /// [`outram_foam_appbuilder_lib::genfoam::neutronics::sp3::Sp3Neutronics`]
    /// via `with_cross_sections(..)` from the Stage-1/2 MGXS, calls
    /// `solve_eigenvalue()` for the initial critical state, removes 3 of 8 rods,
    /// then advances with `step(dt)` inside the `multi_region` coupling loop while
    /// exchanging fuel/coolant temperatures and power density with the TH model.
    /// Blocked on the SP3 solver port ([`GENFOAM_SP3_PORT_BEAD`]) and the
    /// mesh-neutronics coupling variant ([`GENFOAM_COUPLING_BEAD`]).
    // TODO(op-fr2.2.4): drive Sp3Neutronics::with_cross_sections + solve_eigenvalue
    // + step(dt) in the multi_region loop for the rod-removal transient. Blocked
    // on op-p6p.15 (SP3 solver port) and op-p6p.8.4 (RegionModel::Sp3 dispatch).
    pub fn run(&self) -> Result<(), WorkflowError> {
        Err(WorkflowError::NotYetImplemented {
            stage: WorkflowStage::Sp3Multiphysics,
            bead: STAGE_BEAD,
        })
    }
}
