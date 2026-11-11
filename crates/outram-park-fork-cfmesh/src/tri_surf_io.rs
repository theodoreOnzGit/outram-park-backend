// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/meshes/triSurf/triSurf.C (readFromFTR,
//     readFromFMS, readSurface, topologyCheck)
//   meshLibrary/utilities/containers/LongList/LongList.C (operator>>)
//   meshLibrary/utilities/meshes/meshSubset/meshSubsetI.H (operator>>)
// and from OpenFOAM-v2506 (www.openfoam.com), GPL-3.0-or-later, for the
// stream formats of `List<T>` (`N(...)`, `N{...}`), `geometricSurfacePatch`
// and `labelledTri`.
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's own surface files (gh:#1476): `triSurf::readFromFMS` and
//! `readFromFTR`, the `surfaceFile` formats the generators read besides
//! STL.
//!
//! - **`.ftr`**: patches, points, triangles.
//! - **`.fms`**: the same, then feature edges, point subsets, facet subsets
//!   and feature-edge subsets.
//!
//! A patch is `name geometricType`, a point `(x y z)`, a triangle
//! `((a b c) region)`, a feature edge `(a b)` and a subset
//! `name type (labels)`. A list is `N ( … )`, `N { element }` (uniform)
//! or `N()`; the tokens are `outram_foam_basic_lib::io::dict::tokenize`'s,
//! the workspace's one OpenFOAM tokeniser. The surface is built with
//! [`TriSurf::from_parts`] and then checked as `triSurf::topologyCheck`
//! does (an out-of-range label is an error; a triangle with a repeated
//! point is a warning).
//!
//! STL is read by `outram-foam-mesh`'s port of OpenFOAM's `STLReader`
//! (feature `surface-file`, [`from_stl_zoned`]); [`read_surface_bytes`]
//! dispatches on the extension as `triSurf::readSurface` does.
//!
//! Not ported: the binary (`IOstream::BINARY`) variants of these files and
//! gzip-compressed input.

use crate::math::Vec3;
use crate::mesh_octree::{SurfacePatch, TriSurf};
use outram_foam_basic_lib::io::dict::{tokenize, Token};

/// A forward cursor over the tokens of a cfMesh stream file.
struct Cursor {
    tokens: Vec<Token>,
    pos: usize,
    context: String,
}

impl Cursor {
    fn new(text: &str, context: &str) -> Self {
        Cursor { tokens: tokenize(text), pos: 0, context: context.into() }
    }

    fn err(&self, m: &str) -> String {
        format!("{}: token {}: {m}", self.context, self.pos)
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|t| t.text.as_str())
    }

    fn next(&mut self) -> Result<String, String> {
        let t = self.tokens.get(self.pos).map(|t| t.text.clone()).ok_or_else(|| self.err("unexpected end of file"))?;
        self.pos += 1;
        Ok(t)
    }

    fn expect(&mut self, s: &str) -> Result<(), String> {
        let t = self.next()?;
        if t == s {
            Ok(())
        } else {
            Err(self.err(&format!("expected `{s}`, found `{t}`")))
        }
    }

    fn label(&mut self) -> Result<usize, String> {
        let t = self.next()?;
        t.parse::<usize>().map_err(|_| self.err(&format!("expected a label, found `{t}`")))
    }

    fn scalar(&mut self) -> Result<f64, String> {
        let t = self.next()?;
        t.parse::<f64>().map_err(|_| self.err(&format!("expected a scalar, found `{t}`")))
    }

    /// `List<T>::operator>>`: `N ( e … )`, `N { e }` or `( e … )`.
    fn list<T, F: FnMut(&mut Cursor) -> Result<T, String>>(&mut self, mut elem: F) -> Result<Vec<T>, String>
    where
        T: Clone,
    {
        if self.peek() == Some("(") {
            self.next()?;
            let mut out = Vec::new();
            while self.peek() != Some(")") {
                if self.peek().is_none() {
                    return Err(self.err("unexpected end of file inside `( … )`"));
                }
                out.push(elem(self)?);
            }
            self.next()?;
            return Ok(out);
        }
        let n = self.label()?;
        match self.next()?.as_str() {
            "(" => {
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(elem(self)?);
                }
                self.expect(")")?;
                Ok(out)
            }
            "{" => {
                let e = elem(self)?;
                self.expect("}")?;
                Ok(vec![e; n])
            }
            t => Err(self.err(&format!("expected `(` or `{{` after a list size, found `{t}`"))),
        }
    }

    fn point(&mut self) -> Result<Vec3, String> {
        self.expect("(")?;
        let p = Vec3::new(self.scalar()?, self.scalar()?, self.scalar()?);
        self.expect(")")?;
        Ok(p)
    }

    fn patch(&mut self) -> Result<SurfacePatch, String> {
        let name = self.next()?;
        let geometric_type = self.next()?;
        Ok(SurfacePatch { name, geometric_type })
    }

    /// `labelledTri`: `((a b c) region)`.
    fn labelled_tri(&mut self) -> Result<([usize; 3], usize), String> {
        self.expect("(")?;
        self.expect("(")?;
        let t = [self.label()?, self.label()?, self.label()?];
        self.expect(")")?;
        let r = self.label()?;
        self.expect(")")?;
        Ok((t, r))
    }

    fn edge(&mut self) -> Result<[usize; 2], String> {
        self.expect("(")?;
        let e = [self.label()?, self.label()?];
        self.expect(")")?;
        Ok(e)
    }

    /// `meshSubset`: `name type (labels)`.
    fn subset(&mut self) -> Result<(String, Vec<usize>), String> {
        let name = self.next()?;
        let _type = self.label()?;
        let data = self.list(|c| c.label())?;
        Ok((name, data))
    }
}

/// A surface file read into a [`TriSurf`], with `topologyCheck`'s warnings.
#[derive(Debug, Clone)]
pub struct SurfaceRead {
    /// The surface.
    pub surface: TriSurf,
    /// `topologyCheck`'s warnings (a triangle or feature edge with a
    /// repeated point).
    pub warnings: Vec<String>,
}

/// `triSurf::readFromFTR`: patches, points, triangles.
///
/// # Errors
/// A malformed stream, or a label out of range (`topologyCheck`).
pub fn read_ftr(text: &str) -> Result<SurfaceRead, String> {
    let mut c = Cursor::new(text, "ftr");
    let patches = c.list(|c| c.patch())?;
    let points = c.list(|c| c.point())?;
    let tris = c.list(|c| c.labelled_tri())?;
    build(points, tris, patches, Vec::new(), [Vec::new(), Vec::new(), Vec::new()])
}

/// `triSurf::readFromFMS`: patches, points, triangles, feature edges and
/// the point, facet and feature-edge subsets.
///
/// # Errors
/// A malformed stream, or a label out of range (`topologyCheck`).
pub fn read_fms(text: &str) -> Result<SurfaceRead, String> {
    let mut c = Cursor::new(text, "fms");
    let patches = c.list(|c| c.patch())?;
    let points = c.list(|c| c.point())?;
    let tris = c.list(|c| c.labelled_tri())?;
    let feature_edges = c.list(|c| c.edge())?;
    let point_subsets = c.list(|c| c.subset())?;
    let facet_subsets = c.list(|c| c.subset())?;
    let edge_subsets = c.list(|c| c.subset())?;
    build(points, tris, patches, feature_edges, [point_subsets, facet_subsets, edge_subsets])
}

type Subsets = Vec<(String, Vec<usize>)>;

/// The surface from its parts, with `topologyCheck`.
fn build(points: Vec<Vec3>, tris: Vec<([usize; 3], usize)>, patches: Vec<SurfacePatch>, feature_edges: Vec<[usize; 2]>, subsets: [Subsets; 3]) -> Result<SurfaceRead, String> {
    let np = points.len();
    let mut warnings = Vec::new();
    for (i, p) in points.iter().enumerate() {
        if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
            return Err(format!("Point {i} has invalid coordinates"));
        }
    }
    for (f, (t, r)) in tris.iter().enumerate() {
        for k in 0..3 {
            if t[k] >= np {
                return Err(format!("Point {} in triangle {f} is out of scope 0 {np}", t[k]));
            }
            if t[k] == t[(k + 1) % 3] || t[k] == t[(k + 2) % 3] {
                warnings.push(format!("Triangle {f} {t:?} has duplicated points. This may cause problems in the meshing process!"));
                break;
            }
        }
        if *r >= patches.len().max(1) {
            return Err(format!("triangle {f} names patch {r} of {}", patches.len()));
        }
    }
    for (e, fe) in feature_edges.iter().enumerate() {
        if fe[0] >= np || fe[1] >= np {
            return Err(format!("Feature edge {e} {fe:?} is out of scope 0 {np}"));
        }
        if fe[0] == fe[1] {
            warnings.push(format!("Feature edge {e} {fe:?} has duplicated points. This may cause problems in the meshing process!"));
        }
    }
    let n_facets = tris.len();
    let n_edges = feature_edges.len();
    let triangles: Vec<[usize; 3]> = tris.iter().map(|x| x.0).collect();
    let regions: Vec<usize> = tris.iter().map(|x| x.1).collect();
    let mut s = TriSurf::from_parts(points, triangles, regions, patches, feature_edges);
    let [ps, fs, es] = subsets;
    for (group, list, limit, what) in [(&mut s.point_subsets, ps, np, "point"), (&mut s.facet_subsets, fs, n_facets, "facet"), (&mut s.edge_subsets, es, n_edges, "feature edge")] {
        for (name, data) in list {
            let id = group.add(&name);
            for d in data {
                if d >= limit {
                    return Err(format!("{what} {d} in subset {name} is out of scope 0 {limit}"));
                }
                group.add_element(id, d);
            }
        }
    }
    Ok(SurfaceRead { surface: s, warnings })
}

/// Which reader `triSurf::readSurface` picks for a file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFormat {
    /// `.fms` / `.FMS`.
    Fms,
    /// `.ftr` / `.FTR`.
    Ftr,
    /// Anything else: OpenFOAM's `triSurface` reader (STL here).
    TriSurface,
}

/// `triSurf::readSurface`'s choice by extension.
pub fn surface_format(file_name: &str) -> SurfaceFormat {
    let ext = file_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "fms" | "FMS" => SurfaceFormat::Fms,
        "ftr" | "FTR" => SurfaceFormat::Ftr,
        _ => SurfaceFormat::TriSurface,
    }
}

/// `triSurf(fileName)` on the file's bytes: FMS, FTR, or (feature
/// `surface-file`) STL through OpenFOAM's `triSurface::readSTL`. Without the
/// feature an STL is refused with a message saying so.
///
/// # Errors
/// A malformed file, an unsupported extension (OpenFOAM's `triSurface` reads
/// more formats than STL; only STL is ported), or STL without the feature.
pub fn read_surface_bytes(file_name: &str, bytes: &[u8]) -> Result<SurfaceRead, String> {
    match surface_format(file_name) {
        SurfaceFormat::Fms => read_fms(&String::from_utf8_lossy(bytes)),
        SurfaceFormat::Ftr => read_ftr(&String::from_utf8_lossy(bytes)),
        SurfaceFormat::TriSurface => {
            let lower = file_name.to_ascii_lowercase();
            if !(lower.ends_with(".stl") || lower.ends_with(".stlb")) {
                return Err(format!("surfaceFile {file_name}: only .stl, .fms and .ftr are read by this port (OpenFOAM's triSurface reads more formats)"));
            }
            read_stl_bytes(bytes)
        }
    }
}

#[cfg(any(feature = "surface-file", test))]
fn read_stl_bytes(bytes: &[u8]) -> Result<SurfaceRead, String> {
    let z = outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes(bytes).map_err(|e| e.to_string())?;
    Ok(SurfaceRead { surface: from_stl_zoned(&z), warnings: Vec::new() })
}

#[cfg(not(any(feature = "surface-file", test)))]
fn read_stl_bytes(_bytes: &[u8]) -> Result<SurfaceRead, String> {
    Err("reading STL needs this crate's `surface-file` feature (OpenFOAM's STLReader lives in outram-foam-mesh)".into())
}

/// `triSurf::readSurface`'s copy of a `triSurface`: points, triangles with
/// their zone as region, and the patches. `triSurface(fileName)` ends with
/// `setDefaultPatches` (OpenFOAM-v2506 `triSurface.C`, `calcPatches`): a
/// patch keeps its solid name (ASCII) or takes `surfacePatch::defaultName`,
/// `patch<i>` (binary), and every patch gets the geometric type `empty`
/// (`emptyType`), since `readSTL` sets none. The generators'
/// `renameBoundaryPatches` later turns `empty` into `wall`.
#[cfg(any(feature = "surface-file", test))]
pub fn from_stl_zoned(z: &outram_foam_mesh::snappy_hex_mesh::stl::StlZoned) -> TriSurf {
    let points: Vec<Vec3> = z.points.iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect();
    let patches: Vec<SurfacePatch> = (0..z.n_zones)
        .map(|i| SurfacePatch { name: z.names.get(i).cloned().unwrap_or_else(|| format!("patch{i}")), geometric_type: "empty".to_string() })
        .collect();
    TriSurf::from_parts(points, z.triangles.clone(), z.zones.clone(), patches, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `hatOctree` tutorial's `geom.fms` (cfMesh `e944f6ba695f`
    /// `tutorials/cartesian2DMesh/hatOctree/geom.fms`, 334 bytes),
    /// verbatim.
    pub(crate) const HAT_FMS: &str = include_str!("../test_data/cfmesh_tutorials/cartesian2DMesh/hatOctree/geom.fms");

    /// The hat surface reads as upstream lists it: 5 wall patches, 10
    /// points, 10 triangles two per patch, no feature edges or subsets.
    #[test]
    fn hat_fms_reads() {
        let r = read_fms(HAT_FMS).unwrap();
        let s = &r.surface;
        assert!(r.warnings.is_empty());
        assert_eq!(s.patch_names(), ["left", "bottom", "right", "top0", "top1"]);
        assert!(s.patches.iter().all(|p| p.geometric_type == "wall"));
        assert_eq!(s.points.len(), 10);
        assert_eq!(s.points[3], Vec3::new(0.0, 0.5, 0.0));
        assert_eq!(s.triangles.len(), 10);
        assert_eq!(s.triangles[1], [0, 6, 5]);
        assert_eq!(s.regions, vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);
        assert!(s.feature_edges.is_empty());
        assert!(s.facet_subsets.indices().is_empty());
    }

    /// FTR, a uniform list, feature edges and subsets in an FMS, and the
    /// `topologyCheck` errors and warnings.
    #[test]
    fn ftr_fms_lists_and_checks() {
        let ftr = "1(p wall) 3((0 0 0) (1 0 0) (0 1 0)) 1(((0 1 2) 0))";
        let r = read_ftr(ftr).unwrap();
        assert_eq!(r.surface.triangles, vec![[0, 1, 2]]);
        let fms = "1(p wall) 3((0 0 0) (1 0 0) (0 1 0)) 2{((0 1 2) 0)} 1((0 1)) 0() 1(set 2 1(1)) 1(e 4 (0))";
        let r = read_fms(fms).unwrap();
        assert_eq!(r.surface.triangles.len(), 2);
        assert_eq!(r.surface.feature_edges, vec![[0, 1]]);
        let id = r.surface.facet_subsets.index("set").unwrap();
        assert_eq!(r.surface.facet_subsets.elements(id), vec![1]);
        assert_eq!(r.surface.edge_subsets.elements(r.surface.edge_subsets.index("e").unwrap()), vec![0]);
        assert!(read_ftr("1(p wall) 3((0 0 0) (1 0 0) (0 1 0)) 1(((0 1 5) 0))").is_err());
        let w = read_ftr("1(p wall) 3((0 0 0) (1 0 0) (0 1 0)) 1(((0 1 1) 0))").unwrap();
        assert_eq!(w.warnings.len(), 1);
        assert_eq!(surface_format("a.FMS"), SurfaceFormat::Fms);
        assert_eq!(surface_format("x.ftr"), SurfaceFormat::Ftr);
        assert_eq!(surface_format("geom.stl"), SurfaceFormat::TriSurface);
        assert!(read_surface_bytes("x.obj", b"").is_err());
        assert!(read_surface_bytes("x.fms", HAT_FMS.as_bytes()).is_ok());
    }

    /// STL through OpenFOAM's reader: patch names from the solids,
    /// geometric type `empty` (`setDefaultPatches`).
    #[cfg(any(feature = "surface-file", test))]
    #[test]
    fn stl_reads_with_solid_names() {
        let stl = "solid a\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid a\nsolid b\nfacet normal 0 0 1\nouter loop\nvertex 1 0 0\nvertex 1 1 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid b\n";
        let r = read_surface_bytes("geom.stl", stl.as_bytes()).unwrap();
        assert_eq!(r.surface.patch_names(), ["a", "b"]);
        assert_eq!(r.surface.regions, vec![0, 1]);
        assert_eq!(r.surface.points.len(), 4);
        assert!(r.surface.patches.iter().all(|p| p.geometric_type == "empty"));
    }
}
