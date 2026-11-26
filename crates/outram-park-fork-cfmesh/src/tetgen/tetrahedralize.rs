// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx` `tetrahedralize()`, the part `-p` runs
// after `recoverboundary()`, and the switches of
// `tetgenbehavior::parse_commandline` the `-p` route reads):
//   TetGen — A Quality Tetrahedral Mesh Generator and 3D Delaunay Triangulator
//   https://github.com/libigl/tetgen, commit e05aca7df74e (TetGen 1.6.0)
//   Copyright (C) 2002-2020 Hang Si, Weierstrass Institute for Applied
//   Analysis and Stochastics (WIAS), Berlin.
//   Licence: GNU Affero General Public License v3.0 or later.
//
// This file is part of OUTRAM PARK. It is free software: you can redistribute
// it and/or modify it under the terms of the GNU Affero General Public License
// as published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version. It is distributed WITHOUT
// ANY WARRANTY; see the GNU Affero General Public License for details.

//! The rest of `tetrahedralize()` under `-p` (stage 5, gh:#1794) and the
//! [`Switches`] builder.
//!
//! After `recoverboundary` upstream runs, for `-p` without `-q`, `-D`,
//! `-m`, `-R`, `-i`, `-o2`: `carveholes`; `suppresssteinerpoints` when
//! `subvertstack` is not empty (`-Y/0` turns it off); `recoverdelaunay`;
//! `smooth_vertices` when Steiner points are left in facets or the volume;
//! `improve_mesh`; `jettisonnodes` when points were left unused; and the
//! outputs. [`TetgenMesh::finish_plc`] is that sequence.
//!
//! **Branches left out, and why:** `skipped_facet_list` (self-intersecting
//! input: upstream writes the files and stops with `terminatetetgen(3)`;
//! here [`TetgenError::SelfIntersection`]); `-d` (diagnose, returns before
//! `carveholes`: here [`TetgenMesh::finish_plc`] returns after recovery,
//! with no output); `-YY` with Steiner points on the boundary (upstream
//! writes the files and stops, code 200: here
//! [`TetgenError::InternalError`]); `-m`/`-R` (`meshcoarsening`), `-i`
//! (`insertconstrainedpoints`), `-q` (`delaunayrefinement`, stage 6,
//! gh:#1795), `-o2` (`highorder`), `-c` (`b->convex`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::TetgenError;
use super::mesh::{Behavior, TetgenMesh};
use super::output::TetgenOutput;
use super::plc::PlcInput;
use alloc::string::String;

/// The command-line switches of the `-p` route, as a builder over
/// [`Behavior`]. `Switches::new()` is `-p`; [`Switches::parse`] reads the
/// letters `p`, `z`, `Y` (repeatable), `A`, `M`, `J`, `X`, `d` of a switch
/// string such as `"pzAY"` (upstream's parser takes many more; the ones
/// this route does not port are refused).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Switches {
    /// The behaviour the switches set.
    pub b: Behavior,
}

impl Default for Switches {
    fn default() -> Self {
        Switches::new()
    }
}

impl Switches {
    /// `-p`.
    pub fn new() -> Self {
        Switches {
            b: Behavior {
                plc: true,
                ..Behavior::default()
            },
        }
    }
    /// `-z`: number the output from zero.
    pub fn zero_index(mut self) -> Self {
        self.b.zeroindex = true;
        self
    }
    /// `-Y` (once more each call): do not split the boundary.
    pub fn no_bisect(mut self) -> Self {
        self.b.nobisect += 1;
        self
    }
    /// `-A`: give every tetrahedron its region's attribute.
    pub fn region_attributes(mut self) -> Self {
        self.b.regionattrib = true;
        self
    }
    /// `-M`: do not merge coplanar facets.
    pub fn no_merge_facets(mut self) -> Self {
        self.b.nomergefacet = true;
        self
    }
    /// `-J`: do not jettison unused vertices.
    pub fn no_jettison(mut self) -> Self {
        self.b.nojettison = true;
        self
    }
    /// The [`Behavior`].
    pub fn behavior(&self) -> Behavior {
        self.b
    }
    /// Parse a switch string (`"pzAY"`, with or without a leading `-`).
    pub fn parse(s: &str) -> Result<Switches, String> {
        let mut w = Switches::new();
        for c in s.trim_start_matches('-').chars() {
            match c {
                'p' => w.b.plc = true,
                'z' => w = w.zero_index(),
                'Y' => w = w.no_bisect(),
                'A' => w = w.region_attributes(),
                'M' => w = w.no_merge_facets(),
                'J' => w = w.no_jettison(),
                'X' => w.b.noexact = true,
                'd' => w.b.diagnose = true,
                _ => return Err(alloc::format!("switch -{c} is not ported")),
            }
        }
        Ok(w)
    }
}

impl TetgenMesh {
    /// `tetrahedralize()` after `recoverboundary()` for `-p` (see the
    /// module note), ending with the outputs. `input` gives the volume
    /// holes, regions and point markers (`in->holelist`, `regionlist`,
    /// `pointmarkerlist`). `None` for `-d` (diagnose), as upstream writes
    /// nothing then.
    pub fn finish_plc(&mut self, input: &PlcInput) -> Result<Option<TetgenOutput>, TetgenError> {
        if self.skipped_facet_list.is_some() {
            return Err(TetgenError::SelfIntersection);
        }
        if self.b.diagnose {
            return Ok(None);
        }
        // numelemattrib = in->numberoftetrahedronattributes (0 for a PLC)
        //   + (b->regionattrib > 0). The attributes are all 0.0 so far.
        self.numelemattrib = self.b.regionattrib as usize;
        // parse_commandline: "If no user-specified dihedral angle bound. Use
        //   default ones." Without -q (stage 6) the default 177 becomes 179.9
        //   (`tetgen.cxx:3744`). Applied here so that a `Behavior` built by
        //   hand gives what the command line gives.
        if self.b.optmaxdihedral == 177.0 {
            self.b.optmaxdihedral = 179.9;
        }
        self.pointmarkerlist = input.point_markers.clone();

        self.carveholes(&input.holes, &input.regions)?;

        // (!b->cdt || b->nobisect): -D is not ported, so always true.
        if self.b.supsteiner_level > 0 && !self.subvertstack.is_empty() {
            self.suppresssteinerpoints()?;
        }
        if self.b.nobisect > 1 && (self.st_segref_count > 0 || self.st_facref_count > 0) {
            // -YY: "Boundary contains Steiner points (-YY option)."
            return Err(TetgenError::InternalError);
        }

        // b->plc: recover Delaunayness.
        self.recoverdelaunay()?;

        if self.b.smooth_maxiter > 0 && (self.st_volref_count > 0 || self.st_facref_count > 0) {
            self.smooth_vertices()?;
        }

        self.improve_mesh()?;

        if !self.b.nojettison && (self.dupverts > 0 || self.unuverts > 0) {
            self.jettisonnodes_plc();
        }

        Ok(Some(self.output_plc()))
    }
}
