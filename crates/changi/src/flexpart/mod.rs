// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (FLEXible PARTicle dispersion model), NILU
// Upstream URL     : https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart at the
// crate root. Independent fork; not affiliated with or endorsed by NILU.

//! # `flexpart` — Rust port of FLEXPART's dispersion physics
//!
//! FLEXPART is a Lagrangian particle dispersion model: it releases computational
//! particles, advects them on meteorological fields, perturbs them with
//! parameterised turbulence, and removes mass by dry deposition, wet deposition
//! and radioactive decay.
//!
//! ## What is ported so far
//!
//! This module currently covers the **surface-layer and deposition scalar
//! kernels** — the pure functions that turn meteorological surface fields into
//! turbulence scales and aerosol deposition properties. These were chosen first
//! because every one of them depends only on `par_mod` constants in upstream
//! (verified by inspecting their `use` statements), so each can be called
//! directly from a Fortran driver and verified against the real FLEXPART with
//! no meteorological input files, no GRIB reader and no NetCDF.
//!
//! | Submodule | Upstream files | Content |
//! |---|---|---|
//! | [`constants`] | `par_mod.f90` | Physical constants |
//! | [`thermo`] | `ew.f90`, `dynamic_viscosity.f90` | Saturation vapour pressure, dynamic viscosity |
//! | [`surface_layer`] | `psim.f90`, `psih.f90`, `scalev.f90`, `obukhov.f90`, `raerod.f90` | Monin–Obukhov similarity, friction velocity, aerodynamic resistance |
//! | [`aerosol`] | `part0.f90` | Lognormal size distribution, settling, Cunningham, Schmidt |
//! | [`decay`] | `readreleases.f90`, `timemanager.f90` | Radioactive decay |
//!
//! ## What is NOT ported
//!
//! Everything else, which is most of FLEXPART: the particle advection loop
//! (`advance.f90`), the Hanna turbulence parameterisation, the convective
//! boundary-layer scheme (`cbl.f90`), wet scavenging (`wetdepo.f90`,
//! `get_wetscav.f90`), the Richardson-number mixing-height diagnostic, the
//! GRIB/NetCDF meteorological readers, the output grids, and the OH-reaction
//! chemistry. Do not read this module as "FLEXPART in Rust" — it is the first
//! verified slice of one.
//!
//! ## Precision, and why results differ from a stock FLEXPART build
//!
//! FLEXPART's makefile passes no `-fdefault-real-8`, so its default `real` is
//! **single precision**. This port computes in `f64`. Agreement with an
//! as-shipped FLEXPART build is therefore bounded at roughly `1e-7` relative by
//! upstream's own storage format, not by any translation error. The
//! verification measures both: against a `-fdefault-real-8` build of the same
//! Fortran the port agrees to near machine precision, which is what isolates
//! the translation from the precision. See `docs/flexpart-code-to-code.md`.
//!
//! ## Intended use
//!
//! Research, education and verification/validation only. See the crate-level
//! documentation for the scope limits, which are binding.

pub mod aerosol;
pub mod constants;
pub mod decay;
pub mod surface_layer;
pub mod thermo;
