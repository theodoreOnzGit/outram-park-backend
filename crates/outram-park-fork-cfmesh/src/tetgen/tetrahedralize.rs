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
//! [`TetgenError::InternalError`]); ~~`-m`/`-R` (`meshcoarsening`)~~
//! (**`-m` ported 2026-11-26, gh:#1825:** `meshcoarsening` runs after the
//! Steiner-point suppression, [`super::sizing`]; `-R` is not ported), `-i`
//! (`insertconstrainedpoints`), ~~`-q` (`delaunayrefinement`, stage 6,
//! gh:#1795)~~ (**ported in stage 6, gh:#1795:** `-q` and `-a#` run
//! `delaunayrefinement` after `recoverdelaunay`, [`super::refine`]; `-aa`,
//! per-region volumes, is not ported), `-o2` (`highorder`), `-c`
//! (`b->convex`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::TetgenError;
use super::mesh::{Behavior, TetgenMesh};
use super::output::TetgenOutput;
use super::plc::PlcInput;
use alloc::string::String;
use petir::mathf::RealMath;

/// The command-line switches of the `-p` route, as a builder over
/// [`Behavior`]. `Switches::new()` is `-p`; [`Switches::parse`] reads the
/// letters `p`, `z`, `Y` (repeatable), `A`, `M`, `J`, `X`, `d`, (stage
/// 6, gh:#1795) `q` with its optional `ratio/angle` and `a` with its
/// volume, and (gh:#1825) `m` with its optional scale, of a switch string
/// such as `"pzAY"` or `"pq1.414a0.1mz"`
/// (upstream's parser takes many more; the ones this route does not port
/// are refused).
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
    /// `-q<ratio>/<min_dihedral>`: Delaunay refinement to a radius-edge
    /// ratio bound (upstream's default 2.0) and a smallest dihedral angle
    /// in degrees (default 3.5). `optmaxdihedral` keeps 177 with `-q`.
    pub fn quality(mut self, ratio: f64, min_dihedral: f64) -> Self {
        self.b.quality = true;
        self.b.minratio = ratio;
        self.b.mindihedral = min_dihedral;
        self
    }
    /// `-a<v>`: no tetrahedron larger than `v` (implies `-q`, upstream's
    /// `parse_commandline`).
    pub fn max_volume(mut self, v: f64) -> Self {
        self.b.fixedvolume = true;
        self.b.maxvolume = v;
        self.b.maxvolume_length = v.r_powf(1.0 / 3.0) / 3.0;
        self.b.quality = true;
        self
    }
    /// `-m<scale>`: apply the input points' mesh sizes
    /// ([`PlcInput::point_mtrs`], a `.mtr` file), each multiplied by
    /// `scale` (upstream's default 1.0; gh:#1825). It does not imply `-q`.
    pub fn metric(mut self, scale: f64) -> Self {
        self.b.metric = true;
        self.b.metric_scale = scale;
        self
    }
    /// The [`Behavior`].
    pub fn behavior(&self) -> Behavior {
        self.b
    }
    /// Parse a switch string (`"pzAY"`, with or without a leading `-`).
    pub fn parse(s: &str) -> Result<Switches, String> {
        let mut w = Switches::new();
        let chars: alloc::vec::Vec<char> = s.trim_start_matches('-').chars().collect();
        let mut j = 0;
        while j < chars.len() {
            let c = chars[j];
            match c {
                'q' => {
                    w.b.quality = true;
                    if let Some(v) = switch_number(&chars, &mut j, "") {
                        w.b.minratio = v;
                    }
                    if j + 1 < chars.len() && (chars[j + 1] == '/' || chars[j + 1] == ',') {
                        j += 1;
                        if let Some(v) = switch_number(&chars, &mut j, "") {
                            w.b.mindihedral = v;
                        }
                    }
                }
                'a' => match switch_number(&chars, &mut j, "e-+") {
                    Some(v) => w = w.max_volume(v),
                    None => return Err("switch -a without a volume (-aa) is not ported".into()),
                },
                'm' => {
                    w.b.metric = true;
                    if let Some(v) = switch_number(&chars, &mut j, "e-+") {
                        w.b.metric_scale = v;
                    }
                }
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
            j += 1;
        }
        Ok(w)
    }
}

/// The number `parse_commandline` reads after the switch letter at `j`:
/// digits and `.` start it, then digits, `.` and the `extra` characters
/// (`e`, `-`, `+` for `-a`); `strtod` of it (0 if it does not parse).
/// `j` is left on its last character. `None` if no number follows.
fn switch_number(chars: &[char], j: &mut usize, extra: &str) -> Option<f64> {
    let start = *j + 1;
    if start >= chars.len() || !(chars[start].is_ascii_digit() || chars[start] == '.') {
        return None;
    }
    let mut k = start;
    while k < chars.len()
        && (chars[k].is_ascii_digit() || chars[k] == '.' || extra.contains(chars[k]))
    {
        k += 1;
    }
    *j = k - 1;
    let t: String = chars[start..k].iter().collect();
    Some(t.parse::<f64>().unwrap_or(0.0))
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
        // parse_commandline: "If '-a' or '-aa' is in use, enable '-q' option
        //   too." (stage 6, gh:#1795; `tetgen.cxx:3736`).
        if self.b.fixedvolume && !self.b.quality {
            self.b.quality = true;
        }
        // parse_commandline: "If no user-specified dihedral angle bound. Use
        //   default ones." Without -q the default 177 becomes 179.9
        //   (`tetgen.cxx:3744`). Applied here so that a `Behavior` built by
        //   hand gives what the command line gives.
        if !self.b.quality && self.b.optmaxdihedral == 177.0 {
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

        if self.b.metric {
            // -m: mesh coarsening (gh:#1825; -R is not ported).
            self.meshcoarsening()?;
        }

        // b->plc: recover Delaunayness.
        self.recoverdelaunay()?;

        if self.b.quality {
            // -q: Delaunay refinement (stage 6, gh:#1795).
            self.delaunayrefinement()?;
        }

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
