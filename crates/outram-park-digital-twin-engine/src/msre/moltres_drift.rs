//! **Moltres's precursor drift on the MSRE's flow path** (maintainer,
//! 2026-11-14: Moltres wired into this crate as an explicit dependency).
//!
//! `outram_park_fork_moltres::precursors::PrecursorDrift` solves the
//! delayed-neutron precursor advection-decay balance by finite volumes
//! (formulation from Moltres's `PrecursorSource`, `PrecursorDecay` and
//! `CoupledScalarAdvection` kernels). Here it runs on a ring mesh whose arc
//! coordinate is transit time around TM-1626's flow path (lower plenum,
//! graphite, upper plenum, external loop: 3.8, 9.4, 3.9, 8.1 s), with the
//! precursor source and the neutron importance from TM-1626 Fig. 4
//! ([`super::circulation::tm1626_path_at`]), the same inputs as
//! [`super::circulation::CirculationModel::LineTm1626`].
//!
//! [`line_loss`] returns the reactivity lost to circulation,
//! `β − <φ†, delayed emission> / <φ†, fission source>`. That is the quantity
//! the simulator's line model holds at full flow, so the two are an
//! independent check on each other: a spatial steady solve against a
//! time-domain ring of plug-flow cells. The test in `moltres_drift/tests.rs`
//! states the methodology, the prediction and the result.
//!
//! This is the same harness as `moltres_loss` in
//! `outram-park-fork-moltres/tests/msre_n4_precursor_drift_tm1626.rs`
//! (gh:#1130), here as library code on this crate's TM-1626 tables, so the
//! digital twin calls Moltres rather than quoting its number.
//!
//! The drift solver, its delayed-family type and the ring mesh are
//! re-exported for callers building a spatial fidelity level.

pub use outram_park_fork_moltres::error::MoltresError;
pub use outram_park_fork_moltres::prelude::{DelayedFamily, PrecursorDrift, RingMesh};

use super::circulation::{tm1626_path_at, TM1626_RESIDENCE_S, TM1626_TABLE1};
use outram_foam_basic_lib::prelude::{Field, SolverSettings, VolScalarField};

/// TM-1626 Table 1's six delayed families as Moltres families.
pub fn tm1626_families() -> Vec<DelayedFamily> {
    TM1626_TABLE1.iter().map(|&(beta, lambda)| DelayedFamily { beta, lambda }).collect()
}

/// The loop time of TM-1626's flow path at full flow \[s\] (25.2 s).
pub fn tm1626_loop_time() -> f64 {
    TM1626_RESIDENCE_S.iter().sum()
}

/// The reactivity lost to circulation \[δk/k\], from Moltres's steady
/// precursor drift on an `cells`-cell ring at `flow` × the full-flow speed
/// (1.0 = TM-1626's residence times, 0.0 = stationary fuel, which loses
/// nothing).
///
/// The precursor diffusivity is zero (pure upwind advection, as in the
/// gh:#1130 harness), and the linear solves are held to 1e-11.
///
/// # Errors
/// [`MoltresError`] for fewer than the mesh's minimum cells, a negative
/// flow, or a linear solve that misses its tolerance.
pub fn line_loss(cells: usize, flow: f64) -> Result<f64, MoltresError> {
    if !(flow >= 0.0) || !flow.is_finite() {
        return Err(MoltresError::InvalidMaterial(format!("flow fraction must be finite and >= 0, got {flow}")));
    }
    let ring = RingMesh::new(tm1626_loop_time(), 1.0, cells)?;
    let families = tm1626_families();
    let drift = PrecursorDrift::new(
        ring.mesh.clone(),
        families.clone(),
        ring.uniform_flux(flow),
        0.0,
        SolverSettings { tolerance: 1e-11, max_iter: 200_000 },
    )?;
    let (source, importance): (Vec<f64>, Vec<f64>) = (0..cells).map(|i| tm1626_path_at(ring.arc_centre(i))).unzip();
    let s_f = VolScalarField::new("Sf", ring.mesh.clone(), Field::new(source.clone()), vec![]);
    let precursors = drift.solve_steady(&s_f, 1.0)?;
    let emission = drift.delayed_source(&precursors);
    let denom: f64 = source.iter().zip(&importance).map(|(a, b)| a * b).sum();
    let num: f64 = emission.internal.as_slice().iter().zip(&importance).map(|(a, b)| a * b).sum();
    let beta: f64 = families.iter().map(|f| f.beta).sum();
    Ok(beta - num / denom)
}

#[cfg(test)]
mod tests;
