// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::solver` — porous TH equation drivers
//!
//! This module will hold the one-phase and two-phase thermal-hydraulics
//! solver drivers: the porous momentum equation (UEqn, with an anisotropic
//! drag tensor Kd assembled from fluid-structure friction closures plus
//! tortuosity-modified turbulent diffusion), the porous energy equation
//! (EEqn, coupled to the structure via the heat-transfer coefficient), the
//! PIMPLE pressure equation (pEqn), and — for two-phase — MULES-limited
//! alpha transport. It reuses basic-lib fvm/fvc assembly and this crate's
//! existing PIMPLE loop scaffolding (`src/solvers/pimple_foam.rs`). The
//! closure correlations themselves (the `closures` module) and the
//! field-state (`phase`/`structure`) do NOT belong here.
//!
//! Ports upstream `src/classes/thermalHydraulics/solvers/**` (the
//! `onePhase`, `onePhaseLegacy`, and `twoPhase` top-level solver drivers).
//!
//! Scaffold-only — no equations are implemented yet. See beads op-p6p.7.11
//! (one-phase) and op-p6p.7.12 (two-phase).

// TODO(genfoam): port onePhase/onePhaseLegacy UEqn+EEqn+pEqn and twoPhase MULES alpha transport (beads op-p6p.7.11, op-p6p.7.12).
