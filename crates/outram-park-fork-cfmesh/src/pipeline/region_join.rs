// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code, NOT a port. It meshes nothing:
// it concatenates meshes that were built one region at a time and records
// which boundary patches face each other, so they can be written as
// OpenFOAM `cyclicAMI` patch pairs (OpenFOAM ESI,
// `src/meshTools/AMIInterpolation/patches/cyclicAMI/cyclicAMIPolyPatch`).

//! **Per-region meshes joined into one mesh by AMI interfaces** (gh:#1861,
//! child of gh:#1858).
//!
//! [`super::multi_region`] fits the interfaces by meshing every region in
//! ONE TetGen run, so the two sides of an interface share their faces. This
//! module is the other route (maintainer, 2026-11-27: "mesh each region
//! separately, then combine them into a multiregion mesh using a AMI
//! interface"):
//!
//! 1. Each region is meshed on its own, by any route that gives a
//!    [`VolumeMesh`] (the single-region pipeline, with its layers grown first).
//!    Its side of every interface is a boundary patch of its own, named by
//!    the caller (by convention `<this region>_to_<other region>`).
//! 2. [`join_regions`] concatenates the meshes. Points are **not** merged,
//!    because the two sides of an interface are different tilings, so no
//!    face is shared. Every cell keeps the index of the region it came from.
//!    An outer patch name that several regions use (`walls`) becomes one
//!    patch. Each interface stays two patches, recorded as an [`AmiPair`].
//! 3. The pair is written as two `cyclicAMI` patches, each naming the other
//!    as `neighbourPatch` (`crate::foam::write_polymesh_with_ami`). A solver
//!    couples them with AMI weights, the overlap areas of the two tilings.
//!
//! The joined mesh leaves in upper-triangular face order
//! ([`crate::renumber_mesh::upper_triangular`], this crate's rule for every
//! public route that returns a mesh). The interfaces are not checked for
//! coincidence here, beyond comparing the two sides' areas: the caller's
//! snapping puts both sides on the true surface, and the AMI weight sums
//! measure how well they overlap.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};

/// One interface between two regions: the two boundary patches of the
/// joined mesh that face each other (written as a `cyclicAMI` pair).
#[derive(Debug, Clone, PartialEq)]
pub struct AmiPair {
    /// The patch on the first region's side.
    pub a: String,
    /// The patch on the second region's side.
    pub b: String,
    /// Area of the `a` side \[m²\].
    pub area_a: f64,
    /// Area of the `b` side \[m²\].
    pub area_b: f64,
}

impl AmiPair {
    /// `area_b / area_a - 1`: how much the two tilings of the interface
    /// differ in area (zero when both lie on the same plane and cover it).
    #[must_use]
    pub fn area_mismatch(&self) -> f64 {
        self.area_b / self.area_a - 1.0
    }
}

/// [`join_regions`]' result.
#[derive(Debug, Clone)]
pub struct JoinedMesh {
    /// The joined mesh: every region's internal faces are internal, every
    /// interface side is a boundary patch.
    pub mesh: VolumeMesh,
    /// Region (index into the input slice) of every cell.
    pub cell_region: Vec<usize>,
    /// The interfaces, in the order asked.
    pub ami: Vec<AmiPair>,
}

/// Total area of patch `name` of `mesh` \[m²\], or `None` without it.
fn patch_area(mesh: &VolumeMesh, name: &str) -> Option<f64> {
    let p = mesh.patches.iter().find(|p| p.name == name)?;
    Some((p.start_face..p.start_face + p.n_faces).map(|f| mesh.face_area_vector(f).length()).sum())
}

/// Join one mesh per region into one mesh, the interfaces `interfaces`
/// (`(a side patch, b side patch)` names) becoming [`AmiPair`]s.
///
/// # Errors
/// If a region mesh fails [`VolumeMesh::validate`], if an interface patch is
/// missing, is in more than one region or is on both sides of a pair, or if
/// an interface patch name is also an outer patch of another region.
pub fn join_regions(regions: &[VolumeMesh], interfaces: &[(String, String)]) -> Result<JoinedMesh, String> {
    if regions.is_empty() {
        return Err("join_regions: no region meshes".into());
    }
    for (k, m) in regions.iter().enumerate() {
        m.validate().map_err(|e| format!("join_regions: region {k}: {e}"))?;
    }
    // Where each interface patch lives.
    let owner_of = |name: &str| -> Result<usize, String> {
        let hits: Vec<usize> = (0..regions.len()).filter(|&k| regions[k].patches.iter().any(|p| p.name == name)).collect();
        match hits.as_slice() {
            [k] => Ok(*k),
            [] => Err(format!("join_regions: interface patch `{name}` is in no region")),
            _ => Err(format!("join_regions: interface patch `{name}` is in regions {hits:?}; an interface side belongs to one region")),
        }
    };
    let mut ami = Vec::with_capacity(interfaces.len());
    for (a, b) in interfaces {
        if a == b {
            return Err(format!("join_regions: interface `{a}` is paired with itself"));
        }
        let (ka, kb) = (owner_of(a)?, owner_of(b)?);
        if ka == kb {
            return Err(format!("join_regions: both sides of `{a}` / `{b}` are in region {ka}"));
        }
        let area_a = patch_area(&regions[ka], a).unwrap_or(0.0);
        let area_b = patch_area(&regions[kb], b).unwrap_or(0.0);
        ami.push(AmiPair { a: a.clone(), b: b.clone(), area_a, area_b });
    }
    let is_interface = |name: &str| interfaces.iter().any(|(a, b)| a == name || b == name);

    // Points and internal faces, region by region.
    let mut points = Vec::new();
    let (mut faces, mut owner, mut neighbour) = (Vec::new(), Vec::new(), Vec::new());
    let mut cell_region = Vec::new();
    let (mut p0s, mut c0s) = (Vec::with_capacity(regions.len()), Vec::with_capacity(regions.len()));
    for (k, m) in regions.iter().enumerate() {
        let (p0, c0) = (points.len(), cell_region.len());
        p0s.push(p0);
        c0s.push(c0);
        points.extend(m.points.iter().copied());
        cell_region.extend(core::iter::repeat_n(k, m.n_cells));
        for f in 0..m.face_count() {
            if let Some(n) = m.neighbour[f] {
                faces.push(m.faces[f].iter().map(|&v| v + p0).collect::<Vec<usize>>());
                owner.push(m.owner[f] + c0);
                neighbour.push(Some(n + c0));
            }
        }
    }
    // Boundary patches, by name in order of first appearance; a name used by
    // several regions is one patch (an interface name may not be).
    let mut names: Vec<String> = Vec::new();
    for m in regions {
        for p in &m.patches {
            if !names.iter().any(|n| *n == p.name) {
                names.push(p.name.clone());
            }
        }
    }
    let mut patches = Vec::with_capacity(names.len());
    for name in &names {
        let start = faces.len();
        for (k, m) in regions.iter().enumerate() {
            for p in m.patches.iter().filter(|p| p.name == *name) {
                for f in p.start_face..p.start_face + p.n_faces {
                    faces.push(m.faces[f].iter().map(|&v| v + p0s[k]).collect::<Vec<usize>>());
                    owner.push(m.owner[f] + c0s[k]);
                    neighbour.push(None);
                }
            }
        }
        if is_interface(name) && regions.iter().filter(|m| m.patches.iter().any(|p| p.name == *name)).count() > 1 {
            return Err(format!("join_regions: interface patch `{name}` is also a patch of another region"));
        }
        patches.push(BoundaryPatch { name: name.clone(), start_face: start, n_faces: faces.len() - start });
    }
    let joined = VolumeMesh {
        points,
        faces,
        owner,
        neighbour,
        n_cells: cell_region.len(),
        patches,
        cells: None,
    };
    joined.validate().map_err(|e| format!("join_regions: joined mesh: {e}"))?;
    let mesh = crate::renumber_mesh::upper_triangular(&joined);
    Ok(JoinedMesh { mesh, cell_region, ami })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::math::Vec3;

    /// `m` with patch `from` renamed `to`.
    fn renamed(mut m: VolumeMesh, from: &str, to: &str) -> VolumeMesh {
        for p in &mut m.patches {
            if p.name == from {
                p.name = to.to_string();
            }
        }
        m
    }

    /// Two boxes side by side at different resolutions, joined across
    /// x = 1: the counts are the sums, every cell keeps its region, the
    /// interface is one pair of equal area (1 m², to round-off), the outer
    /// patch names are shared, the mesh is valid, upper triangular and keeps
    /// the total volume.
    #[test]
    fn two_boxes_join_with_an_ami_pair() {
        let a = cartesian_box(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let b = cartesian_box(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 1.0), [3, 3, 3]);
        let names: Vec<String> = a.patches.iter().map(|p| p.name.clone()).collect();
        let (xmax, xmin) = (
            names.iter().find(|n| n.to_lowercase().contains("xmax")).expect("an xMax patch").clone(),
            names.iter().find(|n| n.to_lowercase().contains("xmin")).expect("an xMin patch").clone(),
        );
        let a = renamed(a, &xmax, "a_to_b");
        let b = renamed(b, &xmin, "b_to_a");
        let (na, nb) = (a.n_cells, b.n_cells);
        let (fa, fb) = (a.face_count(), b.face_count());
        let va = a.total_volume() + b.total_volume();
        let j = join_regions(&[a, b], &[("a_to_b".into(), "b_to_a".into())]).expect("join");
        assert_eq!(j.mesh.n_cells, na + nb);
        assert_eq!(j.mesh.face_count(), fa + fb);
        assert_eq!(j.cell_region.iter().filter(|&&r| r == 0).count(), na);
        assert_eq!(j.cell_region.iter().filter(|&&r| r == 1).count(), nb);
        assert!(j.cell_region[..na].iter().all(|&r| r == 0));
        j.mesh.validate().expect("valid");
        assert!((j.mesh.total_volume() - va).abs() < 1e-12 * va);
        assert!(!crate::poly_mesh_gen_checks::check_upper_triangular(&j.mesh).0, "upper triangular");
        assert_eq!(j.ami.len(), 1);
        assert!((j.ami[0].area_a - 1.0).abs() < 1e-12 && (j.ami[0].area_b - 1.0).abs() < 1e-12, "{:?}", j.ami);
        assert!(j.ami[0].area_mismatch().abs() < 1e-12);
        // One patch per name: the shared outer names merged, both interface sides kept.
        let mut seen: Vec<&str> = j.mesh.patches.iter().map(|p| p.name.as_str()).collect();
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n);
        assert!(seen.contains(&"a_to_b") && seen.contains(&"b_to_a"));
        let side = |name: &str| j.mesh.patches.iter().find(|p| p.name == name).map(|p| p.n_faces).unwrap();
        assert_eq!((side("a_to_b"), side("b_to_a")), (4, 9));
    }

    /// The refusals: a missing side, a side in two regions, a pair inside one
    /// region, a side paired with itself.
    #[test]
    fn join_regions_refuses_a_bad_interface() {
        let a = renamed(cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [1, 1, 1]), "xMax", "a_to_b");
        let b = cartesian_box(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 1.0), [1, 1, 1]);
        let pair = |x: &str, y: &str| vec![(x.to_string(), y.to_string())];
        assert!(join_regions(&[a.clone(), b.clone()], &pair("a_to_b", "b_to_a")).unwrap_err().contains("in no region"));
        let b2 = renamed(b.clone(), "xMin", "a_to_b");
        assert!(join_regions(&[a.clone(), b2], &pair("a_to_b", "zMin")).unwrap_err().contains("belongs to one region"));
        assert!(join_regions(&[a.clone(), renamed(b.clone(), "xMin", "b_to_a")], &pair("a_to_b", "b_to_a")).is_ok());
        // `yMin` is an outer patch of both boxes, so it cannot be a side.
        assert!(join_regions(&[a.clone(), b.clone()], &pair("a_to_b", "yMin")).unwrap_err().contains("is in regions"));
        assert!(join_regions(&[a.clone(), b], &pair("a_to_b", "a_to_b")).unwrap_err().contains("itself"));
        assert!(join_regions(&[], &[]).is_err());
        let a3 = renamed(a, "yMin", "a_side_2");
        assert!(join_regions(&[a3], &pair("a_to_b", "a_side_2")).unwrap_err().contains("region 0"));
    }
}
