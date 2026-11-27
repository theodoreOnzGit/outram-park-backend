// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Re-export: the OpenFOAM `PCG`/`DIC` translation moved to
//! [`crate::genfoam::common::pcg`] (gh:#1361 round 40) so that the neutronics
//! solvers share it with the thermal-hydraulics callers. Every path that
//! named `two_phase::pcg::{pcg_dic, PcgPerformance}` still resolves, to the same
//! function (bit-identical, one implementation).

pub(crate) use crate::genfoam::common::pcg::*;
