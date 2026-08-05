//! Coupled neutronics/thermal-hydraulics drivers, cross-section feedback, and
//! the critical-boron search.
//!
//! # Provenance
//!
//! Translated from Than Yan Ren's (SNRSI) BEDOK MATLAB snapshot
//! (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received 2026-08-05).
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute. Translated with permission; see `docs/bedok-port-scoping.md` §6.
//!
//! | This module | MATLAB source |
//! |---|---|
//! | [`steady`] | `thdiffusion_solverxyz.m` |
//! | [`transient`] | `thdiffusion_solvertimexyz.m` |
//! | [`cross_section_feedback`] | `sigmavalupd3d.m`, `sigmavalupd3d_handler.m` |
//! | [`critical_boron`] | `criticalboron_xyz.m` |
//! | [`sparse`] | MATLAB built-in sparse syntax (`\`, `decomposition`, `spdiags`) |
//! | [`seam`] | *provisional* — the `nodal/` and `th/` interfaces these drivers call |
//!
//! # What "coupling" means here
//!
//! Neutronics and thermal hydraulics are coupled by **Picard iteration**, not
//! by a monolithic Newton solve. The neutronics produces a power distribution;
//! the T-H turns it into fuel and coolant temperatures and a coolant density;
//! [`cross_section_feedback`] turns those back into cross sections; and the
//! cycle repeats. The fields that carry the feedback (coolant density, Doppler
//! temperature, average fuel temperature, wall heat flux) are **under-relaxed**
//! on every pass, because the undamped cycle oscillates between cold/dense and
//! boiling/void states in a BWR.
//!
//! The steady driver ([`steady::solve_coupled_steady`]) runs that cycle to
//! convergence. The transient driver ([`transient::solve_coupled_transient`])
//! starts from it, re-equilibrates the operator it will actually time-step, and
//! then marches the multigroup diffusion equation with six delayed-neutron
//! precursor families, one T-H step per time step. The boron search
//! ([`critical_boron::search_critical_boron`]) wraps a guarded secant around
//! static eigensolves at a frozen T-H state, then refines boron, flux and
//! feedback together.
//!
//! # No Jacobian-free Newton-Krylov solver exists in the snapshot
//!
//! Recorded here because the project's scoping document describes the transient
//! driver as "JFNK-preconditioned", and the case scripts set
//! `params.jfnkprecon`, `params.jfnkrel` and `params.jfnkverb`
//! (`main_exec_diff3d.m:19-21`, `run_neacrpd1t.m:11`), with `params.ptc` and
//! `params.jfnk_max_iter` documented at `main_exec_diff3d.m:50-61`.
//!
//! **No file in the snapshot reads any of those five controls.** The JFNK
//! solver they belong to is `driftflux_solverstatic1d.m`, which is *not in the
//! snapshot*, together with `driftflux_eqnstatic1d5.m`, `enthmix_forward.m`,
//! `enthmix_invert.m` and `bwrchfhottest.m`. The transient driver translated
//! here is a **linear implicit-Euler / exponential-transform time integration
//! with a direct sparse solve per step and Picard feedback coupling** — there
//! is no Newton iteration, no Krylov solver and no preconditioner anywhere in
//! it. Nothing has been invented to fill the gap; see
//! `docs/bedok-port-scoping.md` §1.0 on why gaps are recorded rather than
//! completed during translation.
//!
//! # Status
//!
//! **Unverified.** Nothing here has been run against a benchmark, and the
//! `nodal/` and `th/` calls it makes are [`todo!`] stubs at the time of
//! writing (see [`seam`]). Not for nuclear facility operation, reactor
//! control, licensing, or safety-critical decisions.

pub mod critical_boron;
pub mod cross_section_feedback;
pub mod error;
pub mod seam;
pub mod sparse;
pub mod steady;
pub mod transient;

#[cfg(test)]
mod tests_support;

pub use critical_boron::{search_critical_boron, CriticalBoronOutput};
pub use error::{CouplingError, Result};
pub use steady::{solve_coupled_steady, SteadyOutput};
pub use transient::{solve_coupled_transient, TransientOutput};
