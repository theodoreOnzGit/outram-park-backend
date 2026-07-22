//! Aqueous geochemistry (v1) — bead op-v6s.12.
//!
//! Equilibrium aqueous **speciation** — the reaction-network core for reactive
//! transport (GIRT). Given the total concentration of each PRIMARY (component)
//! species, it finds the free primary and secondary species concentrations that
//! simultaneously satisfy the law of mass action and per-component mass balance,
//! by a Newton solve on log-concentration. Concentrations are in mol/L.
//!
//! See [`ReactionNetwork`] (build + [`ReactionNetwork::speciate`]) and its
//! result [`Speciation`]. The full physical model, the exact Newton residual and
//! Jacobian, and the v1 simplifications (ideal activities `gamma = 1`; no mineral
//! phases, no kinetics, no explicit charge-balance constraint, no water activity)
//! are documented on the [`network`] module.
//!
//! **Deferred to follow-up beads:** Debye–Hückel / Davies activity corrections,
//! mineral precipitation/dissolution with solubility products, kinetic reaction
//! rates, and coupling to the transport solver. Enum dispatch, no trait objects.

pub mod network;

pub use network::{ReactionNetwork, Speciation};
