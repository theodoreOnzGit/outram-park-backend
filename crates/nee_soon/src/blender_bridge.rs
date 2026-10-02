// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Bridge from outram-blender's analytic-CSG fit onto outram-mc-libs' CSG
// geometry. Moved here from outram-blender's `export` module (feature
// `mc-export`) on 2026-10-02, GitHub issue #486. No upstream source.
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

//! **outram-blender -> outram-mc geometry bridge.**
//!
//! [`to_mc_geometry`] fits an authored surface [`Mesh`] to analytic CSG with
//! outram-blender's [`to_csg_primitive`] and maps the result onto
//! outram-mc-libs' CSG types. It lived in outram-blender behind the
//! `mc-export` feature until 2026-10-02, when GitHub issue #486 made
//! outram-mc-libs depend on outram-blender for its geometry description: a
//! blender -> outram-mc edge, even an optional one, would then be a cycle, so
//! the bridge moved up to this coupling crate, which depends on both.

use outram_blender::export::{to_csg_primitive, CsgSurface, ExportError, RegionToken, Sense};
use outram_blender::mesh::Mesh;

/// Convert `mesh` to a real `outram-mc-libs` CSG `Geometry` — the Monte Carlo
/// export bridge.
///
/// (Formerly outram-blender's `mc-export` feature; moved here 2026-10-02,
/// GitHub #486.) Fits `mesh` to analytic CSG via [`to_csg_primitive`],
/// then maps the local-mirror surfaces/region onto `outram-mc-libs`'s real
/// `SurfaceKind` / `RegionToken` and wraps them in a single-cell `Geometry`:
///
/// - each [`CsgSurface`] → the matching `SurfaceKind` variant, tagged
///   `BoundaryType::Transmissive` (an interior surface);
/// - [`Sense::Negative`] → `HalfSpaceSense::Inside` (evaluate `< 0`),
///   [`Sense::Positive`] → `HalfSpaceSense::Outside` (evaluate `> 0`);
/// - the region RPN maps 1:1 onto `outram-mc-libs`'s `RegionToken`;
/// - the region becomes one `Cell` (id `1`) filled `CellFill::Void`, in a single
///   root `Universe`. **The `Void` fill is a placeholder** — the caller assigns
///   the real material/fill and temperature; this bridge exports *geometry*
///   only.
///
/// All fitted surfaces map exactly: box, sphere, and Z-cylinder primitives, plus
/// the **convex-faceted** route — a general [`CsgSurface::Plane`] maps to
/// `outram-mc-libs`' `Plane { a, b, c, d }` (`a·x + b·y + c·z = d`), so an
/// arbitrary convex polyhedron exports as one half-space per face.
///
/// # Errors
///
/// Returns [`ExportError::NotImplemented`] only if `mesh` is not a fittable
/// primitive at all (propagated from [`to_csg_primitive`] — i.e. a non-convex
/// mesh, which has no half-space-intersection CSG; use [`to_faceted_solid`](outram_blender::export::to_faceted_solid) for
/// that boundary representation).
pub fn to_mc_geometry(mesh: &Mesh) -> Result<outram_mc_libs::prelude::Geometry, ExportError> {
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::{Plane, Sphere, XPlane, YPlane, ZCylinder, ZPlane};
    use outram_mc_libs::prelude::{
        BoundaryType, Cell, CellFill, Geometry, HalfSpaceSense, SurfaceKind, Universe,
    };
    use outram_mc_libs::prelude::RegionToken as McToken;

    let desc = to_csg_primitive(mesh)?;

    let bc = BoundaryType::Transmissive;
    let mut surfaces: Vec<SurfaceKind> = Vec::with_capacity(desc.surfaces.len());
    for s in &desc.surfaces {
        let kind = match *s {
            CsgSurface::XPlane { x0 } => SurfaceKind::XPlane(XPlane { x0, bc }),
            CsgSurface::YPlane { y0 } => SurfaceKind::YPlane(YPlane { y0, bc }),
            CsgSurface::ZPlane { z0 } => SurfaceKind::ZPlane(ZPlane { z0, bc }),
            CsgSurface::Sphere { x0, y0, z0, r } => {
                SurfaceKind::Sphere(Sphere { x0, y0, z0, r, bc })
            }
            CsgSurface::ZCylinder { x0, y0, r } => {
                SurfaceKind::ZCylinder(ZCylinder { x0, y0, r, bc })
            }
            // General plane `a·x + b·y + c·z = d` — the convex-faceted route (one
            // plane per face of a convex polyhedron). outram-mc-libs `Plane`
            // matches [`CsgSurface::Plane`] field-for-field, so this maps exactly.
            CsgSurface::Plane { a, b, c, d } => SurfaceKind::Plane(Plane { a, b, c, d, bc }),
        };
        surfaces.push(kind);
    }

    let region: Vec<McToken> = desc
        .region
        .iter()
        .map(|t| match *t {
            RegionToken::Halfspace { surface, sense } => McToken::HalfSpace {
                surface_idx: surface,
                sense: match sense {
                    Sense::Negative => HalfSpaceSense::Inside,
                    Sense::Positive => HalfSpaceSense::Outside,
                },
            },
            RegionToken::Intersection => McToken::Intersection,
            RegionToken::Union => McToken::Union,
            RegionToken::Complement => McToken::Complement,
        })
        .collect();

    // One geometry-only cell; the caller reassigns the fill/material/temperature.
    let cell = Cell {
        id: 1,
        region,
        fill: CellFill::Void,
        temperature: 293.6,
        translation: Position::ZERO,
        tracking: None,
    };
    Ok(Geometry {
        surfaces,
        cells: vec![cell],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_blender::math::Vec3;
    use outram_blender::primitives;

    /// Real-type Monte Carlo bridge (feature `mc-export`): `to_mc_geometry` maps
    /// a fitted box to six `SurfaceKind` planes intersected in one cell, and a
    /// uv-sphere to a single `Sphere` surface.
    #[test]
    fn mc_geometry_export_box_and_sphere() {
        use outram_mc_libs::prelude::{CellFill, RegionToken as McToken, SurfaceKind};

        let geom = to_mc_geometry(&primitives::cube(2.0)).expect("cube exports to MC CSG");
        assert_eq!(geom.surfaces.len(), 6, "box = six planes");
        assert!(
            geom.surfaces.iter().all(|s| matches!(
                s,
                SurfaceKind::XPlane(_) | SurfaceKind::YPlane(_) | SurfaceKind::ZPlane(_)
            )),
            "box surfaces must all be axis planes"
        );
        assert_eq!(geom.cells.len(), 1);
        assert!(
            matches!(geom.cells[0].fill, CellFill::Void),
            "geometry-only cell is Void"
        );
        let halfspaces = geom.cells[0]
            .region
            .iter()
            .filter(|t| matches!(t, McToken::HalfSpace { .. }))
            .count();
        let intersections = geom.cells[0]
            .region
            .iter()
            .filter(|t| matches!(t, McToken::Intersection))
            .count();
        assert_eq!(halfspaces, 6, "six half-spaces");
        assert_eq!(intersections, 5, "combined by five intersections");

        let sph = to_mc_geometry(&primitives::uv_sphere(16, 8, 3.0)).expect("sphere exports");
        assert_eq!(sph.surfaces.len(), 1);
        assert!(matches!(sph.surfaces[0], SurfaceKind::Sphere(_)));
    }

    /// **The MC bridge maps a convex-faceted CSG through**, one general
    /// `Plane` per face.
    ///
    /// ~~The MC bridge honestly refuses a convex-faceted CSG: `outram-mc-libs`
    /// has no general-plane surface, so a stretched octahedron returns
    /// `NotImplemented` rather than a wrong mapping.~~ **CORRECTED
    /// 2026-09-20** — `outram-mc-libs` gained
    /// `geometry::surface::Plane`, and [`to_mc_geometry`] has mapped
    /// `CsgSurface::Plane` onto it field-for-field since `2714d4134`. This
    /// test still asserted the old refusal and had been failing ever since;
    /// it was gated behind blender's `mc-export`, so a plain `cargo test -p
    /// outram-blender` compiled 514 tests without it and never saw it. It
    /// surfaced only when a crate that enabled the feature — `dhoby-ghaut` —
    /// was in the same invocation and Cargo unified features, giving 536.
    /// (Moved here with the bridge on 2026-10-02, GitHub #486; it now runs in
    /// every `cargo test -p nee_soon`.)
    ///
    /// Methodology: the same stretched octahedron as
    /// outram-blender's `export::tests::csg_fit_octahedron_faceted_convex`, which covers the CSG fit
    /// itself; this asserts the **MC mapping** on top of it.
    ///
    /// Results (asserted below): `to_csg_primitive` gives 8 `Plane`s, and
    /// `to_mc_geometry` maps every one of them to a
    /// `SurfaceKind::Plane` carrying the identical `a, b, c, d` — a
    /// field-for-field check rather than merely `is_ok()`, because "it
    /// returned something" is what let the old assertion's replacement go
    /// unnoticed once before.
    #[test]
    fn mc_geometry_maps_a_convex_faceted_plane() {
        use outram_mc_libs::prelude::SurfaceKind;

        let positions = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, -2.0),
        ];
        let faces = vec![
            vec![0, 2, 4],
            vec![2, 1, 4],
            vec![1, 3, 4],
            vec![3, 0, 4],
            vec![2, 0, 5],
            vec![1, 2, 5],
            vec![3, 1, 5],
            vec![0, 3, 5],
        ];
        let octa = Mesh::from_polygons(&positions, &faces);

        let desc = to_csg_primitive(&octa).expect("convex octahedron must fit");
        assert_eq!(desc.surfaces.len(), 8, "octahedron = eight face planes");

        let geom = to_mc_geometry(&octa).expect("the general-plane route must map");
        assert_eq!(
            geom.surfaces.len(),
            8,
            "every face plane must reach the MC geometry"
        );
        // Field-for-field, in order: the mapping claims to be exact, so this
        // checks it is rather than checking it merely happened.
        for (i, (ours, theirs)) in desc.surfaces.iter().zip(geom.surfaces.iter()).enumerate() {
            let CsgSurface::Plane { a, b, c, d } = *ours else {
                panic!("surface {i} is not a general plane: {ours:?}");
            };
            match theirs {
                SurfaceKind::Plane(p) => {
                    assert_eq!((p.a, p.b, p.c, p.d), (a, b, c, d), "plane {i} differs");
                }
                other => panic!("surface {i} mapped to {other:?}, not a Plane"),
            }
        }
    }
}
