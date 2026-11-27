//! The model of the `lumped` rung: a Wigner-Seitz cell of rung 3, a sphere of
//! natural uranium metal at the centre of a graphite sphere, white outer
//! boundary.
//!
//! **Nothing is retyped here.** The compositions (natural uranium metal at
//! 19.05 g/cm³, graphite at 1.73 g/cm³), the cell-average ratio
//! `N_C/N_U` = [`C_PER_U`] and the cell radius come from outram-mc-libs'
//! [`outram_mc_libs::vv::ugraphite`], and the cell is built with the same
//! constructor and arguments as `ws_cell` in
//! `examples/lumped_ugraphite_kinf.rs` (`fhr_pebble_geometry`: a uranium
//! ball, two graphite shells, white outer boundary). The tapes and their
//! processing are the `ugraphite` rung's ([`crate::mc_demo::ugraphite::model`]).

// MOVED 2026-11-25 to `nee_soon::mc_rungs::lumped` (gh:#1742), unchanged.
pub use nee_soon::mc_rungs::lumped::*;
