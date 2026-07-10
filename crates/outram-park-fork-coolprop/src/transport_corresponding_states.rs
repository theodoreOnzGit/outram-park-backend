//! General **corresponding-states** transport models — the fallback viscosity
//! (and, later, conductivity) correlations CoolProp uses for fluids that carry
//! no fluid-specific transport fit.
//!
//! **Scaffold only (bead op-kbc.17).** [`corresponding_states_viscosity`]
//! currently returns `None`, so behaviour is unchanged (these fluids already
//! return `None` from [`crate::transport::viscosity`]).
//!
//! # Why this exists
//!
//! ~6 fluids (cyclopentane, isopentane, ethylbenzene, R1234yf, R1234ze(E),
//! R152A) have no dedicated viscosity correlation in CoolProp — they use a
//! generalised corresponding-states method keyed off a reference fluid. Until
//! these are implemented [`crate::transport::viscosity`] returns `None` for
//! them (never a wrong number). Closing this is the last viscosity-coverage gap.
#![allow(dead_code, unused_variables)] // TODO(op-kbc.17): drop once implemented

use crate::fluid::Fluid;

/// Which corresponding-states method a fluid uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrStatesModel {
    /// Chung–Lee–Starling (1988) — generalised viscosity from `T_c`, `ρ_c`,
    /// `ω`, dipole moment and an association factor. CoolProp `viscosity_Chung`.
    Chung,
    /// Extended corresponding states (ECS): shape factors map the fluid onto a
    /// reference fluid's transport surface. CoolProp `viscosity_ECS`.
    Ecs,
    /// `ρ·s_r` (residual-entropy) corresponding-states scaling. CoolProp
    /// `viscosity_rhosr`.
    RhosrCs,
}

/// Dynamic viscosity `μ` \[Pa·s\] of `fluid` at temperature `t` \[K\] and mass
/// density `rho` \[kg/m³\] from its corresponding-states model, or `None` if the
/// fluid has no such model (or it is not yet implemented).
///
/// This is the entry point [`crate::transport::viscosity`] will fall back to
/// once the models below are implemented and wired into `Fluid::transport()`.
pub fn corresponding_states_viscosity(fluid: Fluid, t: f64, rho: f64) -> Option<f64> {
    let model = corr_states_model(fluid)?;
    match model {
        // TODO(op-kbc.17): implement each method; until then, no wrong numbers.
        CorrStatesModel::Chung => None,
        CorrStatesModel::Ecs => None,
        CorrStatesModel::RhosrCs => None,
    }
}

/// The corresponding-states viscosity model assigned to `fluid`, if any.
///
/// TODO(op-kbc.17): populate from the CoolProp transport JSON (`viscosity.type`
/// ∈ {`Chung`, `ECS`, `rhosr`}) via the transport codegen.
fn corr_states_model(fluid: Fluid) -> Option<CorrStatesModel> {
    None
}
