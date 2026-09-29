//! Chemical attack on HTGR graphite and fuel, as cited closed-form rate laws.
//!
//! **Not a port.** Each function transcribes one published correlation, with
//! its source, table and validity range at its definition. Added 2026-09-29
//! for `htgr_sim_v1`'s water-ingress stage (gh:#401).
//!
//! - [`graphite_steam`] -- IG-110 oxidation by steam, `C + H2O -> CO + H2`
//!   (Wang & Sun 2023, Boltzmann-enhanced Langmuir-Hinshelwood fit).
//! - [`kernel_hydrolysis`] -- the burst of stored fission gas from exposed
//!   UO2 kernels meeting water vapour (IAEA-TECDOC-978 Eq. 5-2).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): kinetic fits,
//! each only valid inside its stated range, and the caller is told when it
//! leaves it.

pub mod graphite_steam;
pub mod kernel_hydrolysis;
