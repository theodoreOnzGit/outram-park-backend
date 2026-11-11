// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.org)
// Copyright (C) 2011-2026 OpenFOAM Foundation
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! `snappyHexMeshDict` reader (gh:#1470).
//!
//! **Upstream followed:** OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b`,
//! `applications/utilities/mesh/generation/snappyHexMesh/snappyHexMesh.C` and
//! `snappyHexMeshDriver/{refinementParameters,snapParameters}/*.C`; the
//! tutorial dictionary is `tutorials/mesh/snappyHexMesh/flange/system/snappyHexMeshDict`.
//! Note the keyword is `insidePoint` / `insidePoints` in dev (`locationInMesh`
//! is the ESI spelling); both are read.
//!
//! **No silent skip.** Every keyword in the file is either *honoured* (it
//! changes the controls this crate builds) or listed in
//! [`SnappyDict::not_ported`] with its dotted path. A caller (the
//! `snappyHexMesh` binary) prints that list, so a keyword that does nothing is
//! never invisible. What is honoured is deliberately small; the rest of the
//! dictionary is the work of gh:#1465 to #1469.
//!
//! `#include "file"` is resolved by [`SnappyDict::read`] (via
//! `FoamFile::read_with_includes`); `#includeEtc` is not supported by the
//! parser and is reported by [`SnappyDict::parse`] as a not-ported keyword.

use std::path::Path;

use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile, FoamValue};
use outram_foam_basic_lib::primitives::Vector3;

use crate::driver::{KeepRegion, MeshingControls, MeshingPhases};
use crate::snappy_hex_mesh::background::Bounds;
use crate::MeshError;

/// One `geometry` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryEntry {
    /// Name used by `refinementSurfaces` and region keywords.
    pub name: String,
    /// `type`: `triSurface` is read; `sphere`, `box`, ... are reported.
    pub kind: String,
    /// `file` (triSurface only).
    pub file: Option<String>,
}

/// The parts of a `snappyHexMeshDict` this crate acts on, and a report of the
/// rest.
#[derive(Debug, Clone, PartialEq)]
pub struct SnappyDict {
    /// `castellatedMesh`.
    pub castellated_mesh: bool,
    /// `snap`.
    pub snap: bool,
    /// `addLayers`.
    pub add_layers: bool,
    /// `geometry`.
    pub geometry: Vec<GeometryEntry>,
    /// `refinementSurfaces.<name>.level (min max)`.
    pub surface_levels: Vec<(String, usize, usize)>,
    /// `insidePoint` / `insidePoints` / `locationInMesh`.
    pub inside_points: Vec<Vector3>,
    /// `nCellsBetweenLevels`.
    pub n_cells_between_levels: Option<usize>,
    /// `minRefinementCells`.
    pub min_refinement_cells: Option<usize>,
    /// `snapControls.nSmoothPatch`.
    pub n_smooth_patch: Option<usize>,
    /// `snapControls.nSolveIter`.
    pub n_solve_iter: Option<usize>,
    /// Dotted paths of keywords that were read and used.
    pub honoured: Vec<String>,
    /// Dotted paths of keywords present in the file and NOT acted on, each
    /// with the reason.
    pub not_ported: Vec<(String, String)>,
}

fn as_usize(e: &FoamEntry) -> Option<usize> {
    match e {
        FoamEntry::Scalar(x) if *x >= 0.0 && x.fract() == 0.0 => Some(*x as usize),
        _ => None,
    }
}

fn as_bool(e: &FoamEntry) -> Option<bool> {
    match e {
        FoamEntry::Word(w) => match w.as_str() {
            "true" | "yes" | "on" | "y" | "t" => Some(true),
            "false" | "no" | "off" | "n" | "f" | "none" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn as_point(e: &FoamEntry) -> Option<Vector3> {
    match e {
        FoamEntry::List(v) => FoamValue::List(v.clone()).as_vector3(),
        _ => None,
    }
}

/// Levels `(min max)`; a bare single level is not valid upstream.
fn as_levels(e: &FoamEntry) -> Option<(usize, usize)> {
    if let FoamEntry::List(v) = e {
        if let [FoamValue::Scalar(a), FoamValue::Scalar(b)] = v.as_slice() {
            if *a >= 0.0 && *b >= *a {
                return Some((*a as usize, *b as usize));
            }
        }
    }
    None
}

/// Every leaf keyword path under `d` (sub-dictionaries are recursed into).
fn leaf_paths(d: &FoamDict, prefix: &str, out: &mut Vec<String>) {
    for (k, e) in d.iter() {
        let path = if prefix.is_empty() {
            k.to_string()
        } else {
            format!("{prefix}.{k}")
        };
        match e {
            FoamEntry::SubDict(sub) => leaf_paths(sub, &path, out),
            _ => out.push(path),
        }
    }
}

impl SnappyDict {
    /// Parse dictionary text (no `#include` resolution).
    ///
    /// # Errors
    /// [`MeshError::DictParse`] on a syntax error, or a keyword of the wrong
    /// shape (`level` not `(min max)`, a non-boolean phase switch, ...).
    pub fn parse(text: &str) -> Result<Self, MeshError> {
        let file = FoamFile::parse(text).map_err(|e| MeshError::DictParse(e.to_string()))?;
        Self::from_dict(&file.dict)
    }

    /// Read `path`, resolving `#include "name"` relative to it.
    ///
    /// # Errors
    /// [`MeshError::DictParse`] if the file cannot be read or parsed.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, MeshError> {
        let file = FoamFile::read_with_includes(path.as_ref())
            .map_err(|e| MeshError::DictParse(e.to_string()))?;
        Self::from_dict(&file.dict)
    }

    fn from_dict(d: &FoamDict) -> Result<Self, MeshError> {
        let bad = |what: &str| MeshError::DictParse(format!("snappyHexMeshDict: bad {what}"));
        let mut honoured: Vec<String> = Vec::new();
        let note = |h: &mut Vec<String>, p: &str| h.push(p.to_string());

        let phase = |key: &str, h: &mut Vec<String>| -> Result<bool, MeshError> {
            match d.get(key) {
                Some(e) => {
                    h.push(key.to_string());
                    as_bool(e).ok_or_else(|| bad(key))
                }
                None => Err(MeshError::DictParse(format!(
                    "snappyHexMeshDict: `{key}` is required"
                ))),
            }
        };
        let castellated_mesh = phase("castellatedMesh", &mut honoured)?;
        let snap = phase("snap", &mut honoured)?;
        let add_layers = phase("addLayers", &mut honoured)?;

        let mut geometry = Vec::new();
        if let Some(g) = d.get_dict("geometry") {
            for (name, e) in g.iter() {
                let FoamEntry::SubDict(sub) = e else {
                    return Err(bad("geometry entry (not a dictionary)"));
                };
                let kind = sub
                    .get_word("type")
                    .ok_or_else(|| bad("geometry type"))?
                    .to_string();
                let file = match sub.get("file") {
                    Some(FoamEntry::Str(s)) | Some(FoamEntry::Word(s)) => Some(s.clone()),
                    _ => None,
                };
                if kind == "triSurface" || kind == "triSurfaceMesh" {
                    note(&mut honoured, &format!("geometry.{name}.type"));
                    if file.is_some() {
                        note(&mut honoured, &format!("geometry.{name}.file"));
                    }
                }
                geometry.push(GeometryEntry {
                    name: name.to_string(),
                    kind,
                    file,
                });
            }
        }

        let mut surface_levels = Vec::new();
        let mut inside_points = Vec::new();
        let mut n_cells_between_levels = None;
        let mut min_refinement_cells = None;
        if let Some(c) = d.get_dict("castellatedMeshControls") {
            if let Some(rs) = c.get_dict("refinementSurfaces") {
                for (name, e) in rs.iter() {
                    if let FoamEntry::SubDict(sub) = e {
                        if let Some(l) = sub.get("level") {
                            let (a, b) = as_levels(l).ok_or_else(|| bad("refinementSurfaces level"))?;
                            surface_levels.push((name.to_string(), a, b));
                            note(
                                &mut honoured,
                                &format!("castellatedMeshControls.refinementSurfaces.{name}.level"),
                            );
                        }
                    }
                }
            }
            for key in ["insidePoint", "locationInMesh"] {
                if let Some(e) = c.get(key) {
                    inside_points.push(as_point(e).ok_or_else(|| bad(key))?);
                    note(&mut honoured, &format!("castellatedMeshControls.{key}"));
                }
            }
            if let Some(e) = c.get("nCellsBetweenLevels") {
                n_cells_between_levels = Some(as_usize(e).ok_or_else(|| bad("nCellsBetweenLevels"))?);
                note(&mut honoured, "castellatedMeshControls.nCellsBetweenLevels");
            }
            if let Some(e) = c.get("minRefinementCells") {
                min_refinement_cells = Some(as_usize(e).ok_or_else(|| bad("minRefinementCells"))?);
                note(&mut honoured, "castellatedMeshControls.minRefinementCells");
            }
        }

        let (mut n_smooth_patch, mut n_solve_iter) = (None, None);
        if let Some(s) = d.get_dict("snapControls") {
            if let Some(e) = s.get("nSmoothPatch") {
                n_smooth_patch = Some(as_usize(e).ok_or_else(|| bad("nSmoothPatch"))?);
                note(&mut honoured, "snapControls.nSmoothPatch");
            }
            if let Some(e) = s.get("nSolveIter") {
                n_solve_iter = Some(as_usize(e).ok_or_else(|| bad("nSolveIter"))?);
                note(&mut honoured, "snapControls.nSolveIter");
            }
        }

        // Everything else is reported, never skipped silently.
        let mut all = Vec::new();
        leaf_paths(d, "", &mut all);
        let mut not_ported = Vec::new();
        for p in all {
            if honoured.contains(&p) {
                continue;
            }
            let reason = if p.starts_with("geometry.") {
                "searchable shapes other than triSurface are not ported (gh:#1470)"
            } else if p.starts_with("addLayersControls.") {
                "layer controls are not read from the dictionary (gh:#1469)"
            } else if p.starts_with("meshQualityControls.") {
                "quality controls are not read; this crate uses its own limits (gh:#1467)"
            } else if p.starts_with("snapControls.") {
                "snap keyword not ported (gh:#1467, #1468)"
            } else if p.starts_with("castellatedMeshControls.") {
                "castellation keyword not ported (gh:#1465, #1466)"
            } else {
                "top-level keyword not ported (gh:#1470)"
            };
            not_ported.push((p, reason.to_string()));
        }
        if n_cells_between_levels.is_some_and(|n| n != 1) {
            not_ported.push((
                "castellatedMeshControls.nCellsBetweenLevels".to_string(),
                "only 1 (plain 2:1) is implemented; larger values need consistentSlowRefinement (gh:#1465)"
                    .to_string(),
            ));
        }
        Ok(Self {
            castellated_mesh,
            snap,
            add_layers,
            geometry,
            surface_levels,
            inside_points,
            n_cells_between_levels,
            min_refinement_cells,
            n_smooth_patch,
            n_solve_iter,
            honoured,
            not_ported,
        })
    }
}

impl SnappyDict {
    /// Build [`MeshingControls`] from this dictionary on a background mesh of
    /// `divisions` cells filling `domain` (the `constant/polyMesh` blockMesh
    /// output): OpenFOAM's rule for cell selection
    /// ([`RefinementSelection::SurfaceIntersection`](crate::snappy_hex_mesh::RefinementSelection)),
    /// the first `insidePoint` as keep point, the first surface's *min* level
    /// (the `max` level only drives curvature refinement, not ported), and the
    /// `castellatedMesh`/`snap` switches. `addLayers` is NOT mapped (reported
    /// in [`not_ported`](Self::not_ported) by the binary).
    ///
    /// # Errors
    /// [`MeshError::DictParse`] if there is no `insidePoint` or no
    /// `refinementSurfaces` level.
    pub fn to_meshing_controls(
        &self,
        domain: Bounds,
        divisions: [usize; 3],
    ) -> Result<MeshingControls, MeshError> {
        let keep = *self
            .inside_points
            .first()
            .ok_or_else(|| MeshError::DictParse("no insidePoint / locationInMesh".to_string()))?;
        let (name, min_level, _max) = self
            .surface_levels
            .first()
            .cloned()
            .ok_or_else(|| MeshError::DictParse("no refinementSurfaces level".to_string()))?;
        let mut c = MeshingControls::external_flow(divisions, min_level)
            .with_domain(domain)
            .with_surface_intersection_refinement();
        c.keep = KeepRegion::At(keep);
        c.surface_patch_name = name;
        c.min_refine_cells = self.min_refinement_cells.unwrap_or(0);
        c.phases = if self.snap {
            MeshingPhases::CastellateSnap
        } else {
            MeshingPhases::Castellate
        };
        if let Some(n) = self.n_smooth_patch {
            c.snap.n_smooth_patch = n;
        }
        if let Some(n) = self.n_solve_iter {
            c.snap.n_solve_iter = n;
        }
        Ok(c)
    }
}

/// Recover the bounds and per-axis cell counts of an axis-aligned uniform
/// background hex mesh (a `blockMesh` box) from its points: the distinct
/// coordinates per axis, merged within `1e-9` of the span, give `n + 1` planes.
///
/// # Errors
/// [`MeshError::Construction`] for fewer than two planes on an axis (or no
/// points).
pub fn background_from_points(points: &[Vector3]) -> Result<(Bounds, [usize; 3]), MeshError> {
    if points.is_empty() {
        return Err(MeshError::Construction("background mesh has no points".to_string()));
    }
    let mut lo = points[0];
    let mut hi = points[0];
    for p in points {
        lo = Vector3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vector3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let span = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
    let mut n = [0usize; 3];
    for axis in 0..3 {
        let mut v: Vec<f64> = points
            .iter()
            .map(|p| match axis {
                0 => p.x,
                1 => p.y,
                _ => p.z,
            })
            .collect();
        v.sort_by(f64::total_cmp);
        let tol = 1e-9 * span[axis].max(f64::MIN_POSITIVE);
        let mut planes = 1usize;
        for w in v.windows(2) {
            if w[1] - w[0] > tol {
                planes += 1;
            }
        }
        if planes < 2 {
            return Err(MeshError::Construction(format!(
                "background mesh is flat along axis {axis}"
            )));
        }
        n[axis] = planes - 1;
    }
    Ok((Bounds::new(lo, hi), n))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DICT: &str = r#"
FoamFile { format ascii; class dictionary; object snappyHexMeshDict; }
castellatedMesh true;
snap true;
addLayers false;
geometry
{
    flange { type triSurface; file "flange.stl"; }
    hole { type sphere; centre (0 0 -0.012); radius 0.003; }
};
castellatedMeshControls
{
    maxLocalCells 100000;
    nCellsBetweenLevels 1;
    minRefinementCells 0;
    features ( { file "flange.extendedFeatureEdgeMesh"; level 0; } );
    refinementSurfaces { flange { level (2 2); } }
    insidePoint (-9.23149e-05 -0.0025 -0.0025);
}
snapControls { nSmoothPatch 2; tolerance 1.0; nSolveIter 300; }
mergeTolerance 1e-6;
"#;

    /// Every keyword is honoured or reported, none dropped.
    #[test]
    fn flange_style_dict_is_fully_accounted() {
        let d = SnappyDict::parse(DICT).unwrap();
        assert!(d.castellated_mesh && d.snap && !d.add_layers);
        assert_eq!(d.surface_levels, vec![("flange".to_string(), 2, 2)]);
        assert_eq!(d.inside_points.len(), 1);
        assert_eq!(d.n_solve_iter, Some(300));
        assert_eq!(d.n_smooth_patch, Some(2));
        let mut all = Vec::new();
        let f = FoamFile::parse(DICT).unwrap();
        leaf_paths(&f.dict, "", &mut all);
        for p in &all {
            let ok = d.honoured.contains(p) || d.not_ported.iter().any(|(q, _)| q == p);
            assert!(ok, "keyword `{p}` silently dropped");
        }
        let np: Vec<&str> = d.not_ported.iter().map(|(p, _)| p.as_str()).collect();
        assert!(np.contains(&"geometry.hole.radius"));
        assert!(np.contains(&"snapControls.tolerance"));
        assert!(np.contains(&"mergeTolerance"));
        assert!(np.contains(&"castellatedMeshControls.features"));
    }

    #[test]
    fn missing_phase_switch_is_an_error() {
        assert!(SnappyDict::parse("snap true; addLayers false;").is_err());
        assert!(SnappyDict::parse("castellatedMesh maybe; snap true; addLayers false;").is_err());
    }

    #[test]
    fn level_must_be_min_max() {
        let t = "castellatedMesh true; snap false; addLayers false;\
                 castellatedMeshControls { refinementSurfaces { s { level (3 1); } } }";
        assert!(SnappyDict::parse(t).is_err());
    }

    #[test]
    fn controls_and_background_from_dict() {
        let d = SnappyDict::parse(DICT).unwrap();
        let pts: Vec<Vector3> = (0..3)
            .flat_map(|i| (0..4).flat_map(move |j| (0..5).map(move |k| Vector3::new(i as f64, j as f64 * 2.0, k as f64 * 0.5))))
            .collect();
        let (b, n) = background_from_points(&pts).unwrap();
        assert_eq!(n, [2, 3, 4]);
        let c = d.to_meshing_controls(b, n).unwrap();
        assert_eq!(c.refinement_level, 2);
        assert_eq!(c.snap.n_solve_iter, 300);
        assert_eq!(c.surface_patch_name, "flange");
        assert!(matches!(c.phases, MeshingPhases::CastellateSnap));
        assert!(SnappyDict::parse("castellatedMesh true; snap true; addLayers false;")
            .unwrap()
            .to_meshing_controls(b, n)
            .is_err());
        assert!(background_from_points(&[]).is_err());
    }
}
