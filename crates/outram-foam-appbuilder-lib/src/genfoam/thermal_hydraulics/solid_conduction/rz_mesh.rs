// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NEW WORK (gh:#1149), not a port. This file is part of OUTRAM PARK,
// distributed under GPL-3.0 like the rest of this crate's GeN-Foam module.

//! The structured R-Z (or planar x-z) mesh of the solid conduction field.
//!
//! **New work, not port (gh:#1149).** It builds an ordinary
//! [`FvMesh`] (wrapped in basic-lib's [`ConductionMesh`], so the same mesh
//! also feeds `outram_foam_basic_lib::steady_conduction`) from a tensor
//! product of radial and axial cell edges.
//!
//! ## Axisymmetric geometry: OpenFOAM's one-cell wedge
//!
//! An axisymmetric case in OpenFOAM is a one-cell-thick wedge of small angle
//! `theta` with two `wedge` patches (front and back). Every cell then has the
//! exact wedge volume and face areas,
//!
//! ```text
//!   V      = (theta/2) (r2^2 - r1^2) dz        (cell)
//!   A_r(r) = theta r dz                        (face at radius r)
//!   A_z    = (theta/2) (r2^2 - r1^2)           (axial face)
//! ```
//!
//! and the finite-volume Laplacian over them is the standard conservative
//! cylindrical scheme. For a **scalar** the `wedge` condition carries no flux:
//! `wedgeFvPatchField` rotates the patch-internal value onto the face, and a
//! rotation is the identity on a scalar, so `snGrad = 0`. Basic-lib's
//! `BoundaryCondition::Wedge` is documented as exactly that zero-gradient
//! stand-in (`fields/boundary/bc.rs`), which is therefore **exact** here; no
//! further port of the wedge transform is needed for conduction. (The
//! OpenFOAM source is not vendored in this checkout; the statement rests on
//! `wedgeFvPatchField.H` as cited in `bc.rs`, not on a fresh reading.)
//!
//! Lengths and positions: `x` is the radius `r`, `z` the axial height, `y`
//! the azimuthal (wedge-normal) direction. Cell centres sit at the mid-radius
//! `(r1 + r2)/2` and mid-height, as basic-lib's `radial_mesh` does, so the
//! cell-to-face distances the Laplacian divides by are `dr/2`, `dz/2`. The
//! wedge angle cancels from every temperature; it only scales volumes,
//! areas and powers.
//!
//! The **planar** geometry (`x` the abscissa, a unit-free `depth` in `y`)
//! uses the same assembly with `A_x = depth dz`, `A_z = depth dx`,
//! `V = depth dx dz`; it is the slab of the transient verification.
//!
//! ## Face and patch order
//!
//! Cells are numbered `c = i + n_r j` (radius fastest). Internal faces are
//! stored per owner in ascending order (the `+r` face, then the `+z` face),
//! so `owner < neighbour` and owners are sorted — the upper-triangular order
//! basic-lib's DIC-preconditioned CG requires. Patches, in order:
//! [`RzPatch::Inner`] (`r = r_min`; zero area on the axis), [`RzPatch::Outer`],
//! [`RzPatch::Bottom`], [`RzPatch::Top`], then the two wedge (or `empty`)
//! patches `front`, `back`.

use std::sync::Arc;

use uom::si::angle::radian;
use uom::si::f64::{Angle, Length};
use uom::si::length::meter;

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_basic_lib::steady_conduction::ConductionMesh;

use super::SolidConductionError;

/// Axisymmetric wedge or planar slab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RzGeometry {
    /// A wedge of the given angle about the `z` axis (OpenFOAM's
    /// axisymmetric convention; `2 pi` gives the full cylinder's volumes).
    Axisymmetric {
        /// Wedge angle.
        wedge_angle: Angle,
    },
    /// A planar x-z slab of the given depth.
    Planar {
        /// Extent normal to the x-z plane.
        depth: Length,
    },
}

/// Cell edges along one axis, strictly increasing, in metres.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisEdges {
    edges: Vec<f64>,
}

impl AxisEdges {
    /// Edges from `start` through consecutive segments, each `(end, cells)`
    /// split into equal cells. Segment ends become cell edges exactly, so a
    /// region boundary or a gap placed at a segment end lies on a face.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for no segments, a segment
    /// with zero cells, or ends not strictly increasing.
    pub fn segments(start: Length, segments: &[(Length, usize)]) -> Result<Self, SolidConductionError> {
        if segments.is_empty() {
            return Err(SolidConductionError::InvalidInput("an axis needs a segment".into()));
        }
        let mut edges = vec![start.get::<meter>()];
        for (k, (end, cells)) in segments.iter().enumerate() {
            let lo = edges[edges.len() - 1];
            let hi = end.get::<meter>();
            if *cells == 0 || !(hi > lo) || !hi.is_finite() {
                return Err(SolidConductionError::InvalidInput(format!(
                    "segment {k}: needs at least one cell and an end above {lo} m (got {cells} cells, {hi} m)"
                )));
            }
            let d = (hi - lo) / *cells as f64;
            for i in 1..=*cells {
                edges.push(if i == *cells { hi } else { lo + d * i as f64 });
            }
        }
        Ok(Self { edges })
    }

    /// `cells` equal cells from `start` to `end`.
    ///
    /// # Errors
    /// As [`Self::segments`].
    pub fn uniform(start: Length, end: Length, cells: usize) -> Result<Self, SolidConductionError> {
        Self::segments(start, &[(end, cells)])
    }

    /// Number of cells.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.edges.len() - 1
    }

    /// Edge `i` (0..=n_cells).
    #[must_use]
    pub fn edge(&self, i: usize) -> Length {
        Length::new::<meter>(self.edges[i])
    }

    /// Edges in metres.
    pub(crate) fn raw(&self) -> &[f64] {
        &self.edges
    }

    /// Cell index containing `x` [m] (edges belong to the upper cell, the
    /// last edge to the last cell), `None` outside.
    fn locate(&self, x: f64) -> Option<usize> {
        let e = &self.edges;
        if !(x >= e[0]) || x > e[e.len() - 1] {
            return None;
        }
        Some((e.partition_point(|v| *v <= x)).saturating_sub(1).min(e.len() - 2))
    }
}

/// The four physical boundaries of an [`RzMesh`] (the wedge front and back
/// patches carry no flux and take no condition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RzPatch {
    /// `r = r_min` (the axis when `r_min = 0`, with zero face area there).
    Inner,
    /// `r = r_max`.
    Outer,
    /// `z = z_min`.
    Bottom,
    /// `z = z_max`.
    Top,
}

impl RzPatch {
    /// All four, in mesh patch order.
    pub const ALL: [Self; 4] = [Self::Inner, Self::Outer, Self::Bottom, Self::Top];

    /// Mesh patch index.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Inner => 0,
            Self::Outer => 1,
            Self::Bottom => 2,
            Self::Top => 3,
        }
    }

    /// Mesh patch name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Inner => "inner",
            Self::Outer => "outer",
            Self::Bottom => "bottom",
            Self::Top => "top",
        }
    }
}

/// One axis-aligned rectangle of a [`RegionMap`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RzRect {
    /// Inner and outer radius.
    pub r: [Length; 2],
    /// Bottom and top.
    pub z: [Length; 2],
    /// Region index of the rectangle.
    pub region: usize,
}

/// A piecewise region map: a default region, overpainted by rectangles in
/// order (the **last** rectangle containing a point wins).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionMap {
    /// Region of any point no rectangle covers.
    pub default_region: usize,
    /// Rectangles, painted in order.
    pub rects: Vec<RzRect>,
}

impl RegionMap {
    /// The region at `(r, z)`.
    #[must_use]
    pub fn region_at(&self, r: Length, z: Length) -> usize {
        self.rects
            .iter()
            .rev()
            .find(|b| r >= b.r[0] && r <= b.r[1] && z >= b.z[0] && z <= b.z[1])
            .map_or(self.default_region, |b| b.region)
    }
}

/// A structured R-Z (or x-z) finite-volume mesh with a region per cell.
#[derive(Debug, Clone)]
pub struct RzMesh {
    conduction: ConductionMesh,
    geometry: RzGeometry,
    r: AxisEdges,
    z: AxisEdges,
}

impl RzMesh {
    /// Build the mesh; each cell's region is `region_at` evaluated at its
    /// centre.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for a negative inner radius on
    /// an axisymmetric mesh, a non-positive wedge angle or depth, or a region
    /// index not below `n_regions`; [`SolidConductionError::Mesh`] if the
    /// assembled mesh fails validation.
    pub fn build<F>(
        geometry: RzGeometry,
        r: AxisEdges,
        z: AxisEdges,
        n_regions: usize,
        region_at: F,
    ) -> Result<Self, SolidConductionError>
    where
        F: Fn(Length, Length) -> usize,
    {
        let (theta, depth) = match geometry {
            RzGeometry::Axisymmetric { wedge_angle } => {
                let t = wedge_angle.get::<radian>();
                if !(t > 0.0) || !t.is_finite() {
                    return Err(SolidConductionError::InvalidInput(format!(
                        "wedge angle must be positive, got {t} rad"
                    )));
                }
                if r.raw()[0] < 0.0 {
                    return Err(SolidConductionError::InvalidInput(
                        "an axisymmetric mesh cannot start at a negative radius".into(),
                    ));
                }
                (Some(t), 0.0)
            }
            RzGeometry::Planar { depth } => {
                let d = depth.get::<meter>();
                if !(d > 0.0) || !d.is_finite() {
                    return Err(SolidConductionError::InvalidInput(format!(
                        "planar depth must be positive, got {d} m"
                    )));
                }
                (None, d)
            }
        };
        let re = r.raw();
        let ze = z.raw();
        let (nr, nz) = (r.n_cells(), z.n_cells());
        let n = nr * nz;
        // Geometry helpers.
        let radial_area = |rf: f64, dz: f64| match theta {
            Some(t) => t * rf * dz,
            None => depth * dz,
        };
        let axial_area = |r1: f64, r2: f64| match theta {
            Some(t) => 0.5 * t * (r2 * r2 - r1 * r1),
            None => depth * (r2 - r1),
        };

        let mut cell_volumes = Vec::with_capacity(n);
        let mut cell_centres = Vec::with_capacity(n);
        let mut cell_zone = Vec::with_capacity(n);
        for j in 0..nz {
            for i in 0..nr {
                let (r1, r2, z1, z2) = (re[i], re[i + 1], ze[j], ze[j + 1]);
                let (rc, zc) = (0.5 * (r1 + r2), 0.5 * (z1 + z2));
                cell_volumes.push(axial_area(r1, r2) * (z2 - z1));
                cell_centres.push(Vector3::new(rc, 0.0, zc));
                let reg = region_at(Length::new::<meter>(rc), Length::new::<meter>(zc));
                if reg >= n_regions {
                    return Err(SolidConductionError::InvalidInput(format!(
                        "region {reg} at (r, z) = ({rc}, {zc}) m is not below n_regions = {n_regions}"
                    )));
                }
                cell_zone.push(reg);
            }
        }

        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        let mut sf = Vec::new();
        let mut cf = Vec::new();
        for j in 0..nz {
            for i in 0..nr {
                let c = i + nr * j;
                let (r1, r2, z1, z2) = (re[i], re[i + 1], ze[j], ze[j + 1]);
                let zc = 0.5 * (z1 + z2);
                let rc = 0.5 * (r1 + r2);
                if i + 1 < nr {
                    owner.push(c);
                    neighbour.push(c + 1);
                    sf.push(Vector3::new(radial_area(r2, z2 - z1), 0.0, 0.0));
                    cf.push(Vector3::new(r2, 0.0, zc));
                }
                if j + 1 < nz {
                    owner.push(c);
                    neighbour.push(c + nr);
                    sf.push(Vector3::new(0.0, 0.0, axial_area(r1, r2)));
                    cf.push(Vector3::new(rc, 0.0, z2));
                }
            }
        }
        let n_internal = owner.len();
        let mut patches = Vec::new();
        let mut push_patch = |name: &str, kind: PatchKind, faces: Vec<(usize, Vector3, Vector3)>, owner: &mut Vec<usize>, sf: &mut Vec<Vector3>, cf: &mut Vec<Vector3>| {
            let start = owner.len();
            let size = faces.len();
            for (c, s, x) in faces {
                owner.push(c);
                sf.push(s);
                cf.push(x);
            }
            patches.push(BoundaryPatch::new(name, start, size, kind));
        };
        let rmin = re[0];
        let rmax = re[nr];
        let zmin = ze[0];
        let zmax = ze[nz];
        let inner: Vec<_> = (0..nz)
            .map(|j| {
                let zc = 0.5 * (ze[j] + ze[j + 1]);
                (nr * j, Vector3::new(-radial_area(rmin, ze[j + 1] - ze[j]), 0.0, 0.0), Vector3::new(rmin, 0.0, zc))
            })
            .collect();
        let outer: Vec<_> = (0..nz)
            .map(|j| {
                let zc = 0.5 * (ze[j] + ze[j + 1]);
                (nr - 1 + nr * j, Vector3::new(radial_area(rmax, ze[j + 1] - ze[j]), 0.0, 0.0), Vector3::new(rmax, 0.0, zc))
            })
            .collect();
        let bottom: Vec<_> = (0..nr)
            .map(|i| {
                let rc = 0.5 * (re[i] + re[i + 1]);
                (i, Vector3::new(0.0, 0.0, -axial_area(re[i], re[i + 1])), Vector3::new(rc, 0.0, zmin))
            })
            .collect();
        let top: Vec<_> = (0..nr)
            .map(|i| {
                let rc = 0.5 * (re[i] + re[i + 1]);
                (i + nr * (nz - 1), Vector3::new(0.0, 0.0, axial_area(re[i], re[i + 1])), Vector3::new(rc, 0.0, zmax))
            })
            .collect();
        // Wedge (or empty) front and back: one face per cell, the meridional
        // cell area dr*dz, normal along -/+y. They carry no flux for a scalar.
        let side_kind = if theta.is_some() { PatchKind::Wedge } else { PatchKind::Empty };
        let side = |sign: f64| -> Vec<(usize, Vector3, Vector3)> {
            (0..n)
                .map(|c| {
                    let (i, j) = (c % nr, c / nr);
                    let a = (re[i + 1] - re[i]) * (ze[j + 1] - ze[j]);
                    (c, Vector3::new(0.0, sign * a, 0.0), cell_centres[c])
                })
                .collect()
        };
        let front = side(-1.0);
        let back = side(1.0);
        push_patch(RzPatch::Inner.name(), PatchKind::Patch, inner, &mut owner, &mut sf, &mut cf);
        push_patch(RzPatch::Outer.name(), PatchKind::Patch, outer, &mut owner, &mut sf, &mut cf);
        push_patch(RzPatch::Bottom.name(), PatchKind::Patch, bottom, &mut owner, &mut sf, &mut cf);
        push_patch(RzPatch::Top.name(), PatchKind::Patch, top, &mut owner, &mut sf, &mut cf);
        push_patch("front", side_kind, front, &mut owner, &mut sf, &mut cf);
        push_patch("back", side_kind, back, &mut owner, &mut sf, &mut cf);

        let mesh = FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n_internal)
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(sf)
            .face_centres(cf)
            .build()
            .map_err(|e| SolidConductionError::Mesh(format!("{e:?}")))?;
        Ok(Self {
            conduction: ConductionMesh {
                mesh: Arc::new(mesh),
                cell_zone,
                n_zones: n_regions,
            },
            geometry,
            r,
            z,
        })
    }

    /// Build with a [`RegionMap`].
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn from_region_map(
        geometry: RzGeometry,
        r: AxisEdges,
        z: AxisEdges,
        n_regions: usize,
        map: &RegionMap,
    ) -> Result<Self, SolidConductionError> {
        Self::build(geometry, r, z, n_regions, |r, z| map.region_at(r, z))
    }

    /// The finite-volume mesh.
    #[must_use]
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.conduction.mesh
    }

    /// The mesh with its cell regions, in basic-lib's form (feeds
    /// `outram_foam_basic_lib::steady_conduction` directly).
    #[must_use]
    pub fn conduction_mesh(&self) -> &ConductionMesh {
        &self.conduction
    }

    /// The geometry.
    #[must_use]
    pub fn geometry(&self) -> RzGeometry {
        self.geometry
    }

    /// Radial edges.
    #[must_use]
    pub fn r_edges(&self) -> &AxisEdges {
        &self.r
    }

    /// Axial edges.
    #[must_use]
    pub fn z_edges(&self) -> &AxisEdges {
        &self.z
    }

    /// Number of cells.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.conduction.cell_zone.len()
    }

    /// Number of regions.
    #[must_use]
    pub fn n_regions(&self) -> usize {
        self.conduction.n_zones
    }

    /// Region of cell `c`.
    #[must_use]
    pub fn cell_region(&self, c: usize) -> usize {
        self.conduction.cell_zone[c]
    }

    /// Cell index of radial column `i`, axial row `j`.
    #[must_use]
    pub fn cell_index(&self, i: usize, j: usize) -> usize {
        i + self.r.n_cells() * j
    }

    /// Centre `(r, z)` of cell `c`.
    #[must_use]
    pub fn cell_centre(&self, c: usize) -> (Length, Length) {
        let p = self.conduction.mesh.cell_centres[c];
        (Length::new::<meter>(p.x), Length::new::<meter>(p.z))
    }

    /// The cell containing `(r, z)`, `None` outside the mesh. This is the
    /// assembled mesh's own lookup, the one the drawings sample.
    #[must_use]
    pub fn cell_at(&self, r: Length, z: Length) -> Option<usize> {
        let i = self.r.locate(r.get::<meter>())?;
        let j = self.z.locate(z.get::<meter>())?;
        Some(self.cell_index(i, j))
    }
}
