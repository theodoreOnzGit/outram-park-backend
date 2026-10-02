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
//! This module covers the **surface-layer, turbulence, deposition and
//! boundary-layer kernels**: the functions that turn meteorological fields
//! into turbulence statistics, deposition velocities and mixing heights. Each
//! is verified against the real FLEXPART, compiled from upstream source, with
//! no meteorological input files, no GRIB reader and no NetCDF. The first set
//! depends only on `par_mod` constants. The stage-1 set (2026-10-02) reads
//! `com_mod`, and its driver writes synthetic fields there (see
//! `docs/flexpart-code-to-code.md`).
//!
//! | Submodule | Upstream files | Content |
//! |---|---|---|
//! | [`constants`] | `par_mod.f90` | Physical constants |
//! | [`thermo`] | `ew.f90`, `dynamic_viscosity.f90` | Saturation vapour pressure, dynamic viscosity |
//! | [`surface_layer`] | `psim.f90`, `psih.f90`, `scalev.f90`, `obukhov.f90`, `raerod.f90` | Monin–Obukhov similarity, friction velocity, aerodynamic resistance |
//! | [`aerosol`] | `part0.f90` | Lognormal size distribution, settling, Cunningham, Schmidt |
//! | [`decay`] | `readreleases.f90`, `timemanager.f90` | Radioactive decay |
//! | [`turbulence`] | `hanna*.f90`, `windalign.f90` | Hanna (1982) turbulence statistics |
//! | [`cbl`] | `cbl.f90` | Skewed convective-boundary-layer drift and diffusion |
//! | [`dry_deposition`] | `getrb.f90`, `getrc.f90`, `partdep.f90`, `getvdep.f90`, `get_settling.f90` | Dry deposition velocity, settling |
//! | [`boundary_layer`] | `pbl_profile.f90`, `richardson.f90`, `qvsat.f90` | Profile fluxes, mixing height, saturation humidity |
//! | [`solar`] | `zenithangle.f90`, `photo_O1D.f90` | Solar zenith angle, O(¹D) photolysis |
//! | [`geodesy`] | `distance.f90`, `distance2.f90` | Great-circle distance |
//! | [`calendar`] | `juldate.f90`, `caldate.f90` | Julian date (day count re-derived; NR provenance) |
//! | [`interpolation`] | `interpol_*.f90` (+ `_nests`) | Met fields at a particle |
//! | [`advance`] | `advance.f90`, `initialize.f90`, `get_vdep_prob.f90` | The Lagrangian particle step |
//! | [`cmapf`] | `cmapf_mod.f90` | Map projections (polar stereographic, Lambert) |
//! | [`coordtrafo`] | `coordtrafo.f90` | Release-point coordinates |
//! | [`met_fields`] | `calcpar*.f90`, `calcpv*.f90` | Surface/PBL parameters, potential vorticity |
//! | [`wet_deposition`] | `interpol_rain*.f90`, `get_wetscav.f90`, `wetdepo.f90`, `wetdepokernel*.f90` | Wet scavenging |
//! | [`oh_chemistry`] | `gethourlyOH.f90`, `ohreaction.f90` | OH reaction |
//! | [`landuse`] | `assignland.f90` | Landuse assignment |
//! | [`concentration`] | `conccalc.f90`, `drydepokernel*.f90` | Concentration and deposition gridding |
//! | [`plume_trajectory`] | `centerofmass.f90`, `clustering.f90`, `plumetraj.f90`, `mean_mod.f90` | Plume statistics |
//! | [`particle_average`] | `partpos_average.f90` | Particle-position averages |
//! | [`convection`] | `convect43c.f90` | Emanuel convection |
//! | [`convmix`] | `calcmatrix.f90`, `redist.f90`, `convmix.f90` | Convective redistribution |
//!
//! ## What is NOT ported
//!
//! ~~The particle advection loop (`advance.f90`), the meteorological
//! interpolation, wet scavenging, the output grids, and the OH reaction.~~
//! **CORRECTED 2026-10-02**: all of those, and the Hanna turbulence, `cbl.f90`
//! and the Richardson mixing height, are ported and verified (gh:#410). Still
//! not ported: the GRIB/NetCDF readers and file writers, `verttransform_*`,
//! particle release and domain filling, the output-grid set-up and
//! `concoutput*` conversion, and `timemanager`.
//! Do not read this module as "FLEXPART in Rust": it is a verified set of its
//! kernels.
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

pub mod advance;
pub mod aerosol;
pub mod boundary_layer;
pub mod calendar;
pub mod cbl;
pub mod cmapf;
pub mod concentration;
pub mod constants;
pub mod convection;
pub mod convmix;
pub mod coordtrafo;
pub mod decay;
pub mod dry_deposition;
pub mod fluxes;
pub mod geodesy;
pub mod initial_condition;
pub mod interpolation;
pub mod landuse;
pub mod met_fields;
pub mod oh_chemistry;
pub mod outgrid;
pub mod particle_average;
pub mod plume_trajectory;
pub mod solar;
pub mod surface_layer;
pub mod thermo;
pub mod turbulence;
pub mod wet_deposition;
