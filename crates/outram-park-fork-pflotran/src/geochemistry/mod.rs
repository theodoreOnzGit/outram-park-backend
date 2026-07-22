//! Aqueous geochemistry (v1) — bead op-v6s.12.
//!
//! **Scaffold.** Equilibrium aqueous speciation (law of mass action + component
//! mass balance, Newton-solved) and simple mineral precipitation/dissolution
//! kinetics — the reaction network for reactive transport (GIRT). Operates on
//! component/species concentrations; coupling to the transport solver is a
//! later step. Enum dispatch, no trait objects.
