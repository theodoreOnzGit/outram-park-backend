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

/// Cells for the plant-coupled drift ([`LoopDrift`]): 504 around the
/// 25.2 s loop, 0.05 s of transit each. Measured 2026-11-14 (gh:#1575,
/// native release): 1008 cells steps 10 s of plant time in 0.145 s, so 504
/// leaves room for a phone's slower wasm. ~~The upwind error grows to about
/// 6e-7 δk/k (first order from the 2520 / 10 080 results)~~ **Measured
/// 2026-11-14:** the steady loss is 0.229218 % at 504 cells against
/// 0.229303 % at 10 080, an upwind error of −8.5e-7 δk/k (the estimate
/// was 6e-7), against the measurement's ±4e-5.
pub const LOOP_DRIFT_CELLS: usize = 504;

/// The most backward-Euler steps one [`LoopDrift::advance`] takes. A longer
/// advance (the GUI's fast speeds) uses longer steps instead, which is
/// stable (implicit) but smears the precursor fronts: the profile is
/// accurate at real time and up to `MAX_STEPS_PER_ADVANCE` cells' transit
/// per request, and smoothed beyond.
pub const MAX_STEPS_PER_ADVANCE: usize = 50;

/// Moltres's precursor drift stepped in time beside the plant: the loop's
/// precursor field follows the plant's fuel flow and fission power.
///
/// The fission source is TM-1626 Fig. 4's shape scaled by the plant's power;
/// the flow is the plant's fuel-flow fraction of full flow (0 = pump
/// stopped). Each [`LoopDrift::advance`] takes backward-Euler steps of one
/// cell's full-flow transit (at most [`MAX_STEPS_PER_ADVANCE`]).
pub struct LoopDrift {
    ring: RingMesh,
    drift: PrecursorDrift,
    shape: Vec<f64>,
    importance: Vec<f64>,
    precursors: Vec<outram_park_fork_moltres::prelude::PrecursorField>,
    flow: f64,
    power: f64,
    time: f64,
}

impl LoopDrift {
    /// The steady loop at `flow` (fraction of full flow, >= 0) and fission
    /// `power` (any unit; the drift is linear in it), on `cells` cells.
    ///
    /// # Errors
    /// [`MoltresError`] for a bad flow, power or mesh, or a failed solve.
    pub fn new(cells: usize, flow: f64, power: f64) -> Result<Self, MoltresError> {
        check_inputs(flow, power)?;
        let ring = RingMesh::new(tm1626_loop_time(), 1.0, cells)?;
        let drift = PrecursorDrift::new(
            ring.mesh.clone(),
            tm1626_families(),
            ring.uniform_flux(flow),
            0.0,
            SolverSettings { tolerance: 1e-11, max_iter: 200_000 },
        )?;
        let (shape, importance): (Vec<f64>, Vec<f64>) = (0..cells).map(|i| tm1626_path_at(ring.arc_centre(i))).unzip();
        let mut d = Self { ring, drift, shape, importance, precursors: Vec::new(), flow, power, time: 0.0 };
        d.precursors = d.drift.solve_steady(&d.source(), 1.0)?;
        Ok(d)
    }

    fn source(&self) -> VolScalarField {
        let s: Vec<f64> = self.shape.iter().map(|x| x * self.power).collect();
        VolScalarField::new("Sf", self.ring.mesh.clone(), Field::new(s), vec![])
    }

    /// Run `seconds` at fuel `flow` and fission `power`, both held over the
    /// interval (the plant's values at its end).
    ///
    /// # Errors
    /// [`MoltresError`] for a bad flow, power or interval, or a failed solve.
    pub fn advance(&mut self, seconds: f64, flow: f64, power: f64) -> Result<(), MoltresError> {
        check_inputs(flow, power)?;
        if !(seconds >= 0.0) || !seconds.is_finite() {
            return Err(MoltresError::InvalidMaterial(format!("interval must be finite and >= 0, got {seconds}")));
        }
        if seconds == 0.0 {
            return Ok(());
        }
        if flow != self.flow {
            self.drift.flow = self.ring.uniform_flux(flow);
            self.flow = flow;
        }
        self.power = power;
        let cell_transit = tm1626_loop_time() / self.shape.len() as f64;
        let n = ((seconds / cell_transit).ceil() as usize).clamp(1, MAX_STEPS_PER_ADVANCE);
        let dt = seconds / n as f64;
        let source = self.source();
        for _ in 0..n {
            self.precursors = self.drift.step(&self.precursors, &source, 1.0, dt)?;
        }
        self.time += seconds;
        Ok(())
    }

    /// Seconds advanced since construction.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The delayed-neutron emission `Σ λ_i C_i` in each cell, in loop order
    /// from the lower-plenum inlet.
    pub fn emission(&self) -> Vec<f64> {
        self.drift.delayed_source(&self.precursors).internal.as_slice().to_vec()
    }

    /// [`LoopDrift::emission`] averaged into `bins` equal arcs (for a GUI).
    pub fn emission_binned(&self, bins: usize) -> Vec<f64> {
        let e = self.emission();
        let bins = bins.clamp(1, e.len());
        (0..bins)
            .map(|b| {
                let (lo, hi) = (b * e.len() / bins, (b + 1) * e.len() / bins);
                e[lo..hi].iter().sum::<f64>() / (hi - lo) as f64
            })
            .collect()
    }

    /// The reactivity lost to circulation now \[δk/k\]:
    /// `β − <φ†, delayed emission> / <φ†, fission source>`, or `None` at
    /// zero power (no source to weigh against). Out of steady state it is
    /// the instantaneous delayed-neutron deficit, which lags the flow by the
    /// precursors' lifetimes.
    pub fn loss(&self) -> Option<f64> {
        let denom: f64 = self.shape.iter().zip(&self.importance).map(|(a, b)| a * b * self.power).sum();
        if !(denom > 0.0) {
            return None;
        }
        let num: f64 = self.emission().iter().zip(&self.importance).map(|(a, b)| a * b).sum();
        let beta: f64 = self.drift.families.iter().map(|f| f.beta).sum();
        Some(beta - num / denom)
    }

    /// Where the regions start along the loop, as fractions of it: lower
    /// plenum, graphite core, upper plenum, external loop (TM-1626 Table 1's
    /// residence times).
    pub fn region_starts() -> [f64; 4] {
        let t = TM1626_RESIDENCE_S;
        let total = tm1626_loop_time();
        [0.0, t[0] / total, (t[0] + t[1]) / total, (t[0] + t[1] + t[2]) / total]
    }
}

fn check_inputs(flow: f64, power: f64) -> Result<(), MoltresError> {
    if !(flow >= 0.0) || !flow.is_finite() {
        return Err(MoltresError::InvalidMaterial(format!("flow fraction must be finite and >= 0, got {flow}")));
    }
    if !(power >= 0.0) || !power.is_finite() {
        return Err(MoltresError::InvalidMaterial(format!("power must be finite and >= 0, got {power}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
