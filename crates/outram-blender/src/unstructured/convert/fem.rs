// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Neutral mesh <-> `farrer-park` (feature `fem-export`): the finite-element
//! family, plus orchestration of farrer-park's own structured generators.
//!
//! **The generators stay in farrer-park** (maintainer default, GitHub #492,
//! 2026-10-03); [`generate`] calls them and converts. farrer-park's node
//! orderings are the neutral mesh's (see [`crate::unstructured::element`]),
//! so the conversion copies connectivity unchanged.
//!
//! farrer-park meshes carry one element type and no named boundaries, so
//! [`from_fem_mesh`] puts every boundary face in `defaultFaces`; name them
//! afterwards with `with_patch_where`. [`to_fem_mesh`] drops patches and
//! zones (farrer-park selects boundary nodes geometrically).

use farrer_park::element::ElementType;
use farrer_park::mesh::{ElemId, Mesh as FemMesh};

use super::ConvertError;
use crate::unstructured::element::{Element, ElementKind, LengthUnit};
use crate::unstructured::mesh::UnstructuredMesh;

fn kind_of(t: ElementType) -> ElementKind {
    match t {
        ElementType::Tri3 => ElementKind::Tri3,
        ElementType::Tri6 => ElementKind::Tri6,
        ElementType::Quad4 => ElementKind::Quad4,
        ElementType::Tet4 => ElementKind::Tet4,
        ElementType::Hex8 => ElementKind::Hex8,
    }
}

fn type_of(k: ElementKind) -> Option<ElementType> {
    match k {
        ElementKind::Tri3 => Some(ElementType::Tri3),
        ElementKind::Tri6 => Some(ElementType::Tri6),
        ElementKind::Quad4 => Some(ElementType::Quad4),
        ElementKind::Tet4 => Some(ElementType::Tet4),
        ElementKind::Hex8 => Some(ElementType::Hex8),
        ElementKind::Polygon | ElementKind::Polyhedron => None,
    }
}

fn name_of(k: ElementKind) -> &'static str {
    match k {
        ElementKind::Tri3 => "Tri3",
        ElementKind::Tri6 => "Tri6",
        ElementKind::Quad4 => "Quad4",
        ElementKind::Tet4 => "Tet4",
        ElementKind::Hex8 => "Hex8",
        ElementKind::Polygon => "Polygon",
        ElementKind::Polyhedron => "Polyhedron",
    }
}

/// farrer-park `Mesh` (metres) -> neutral mesh (metres).
///
/// # Errors
/// A neutral-mesh construction error (an inverted element, say).
pub fn from_fem_mesh(fem: &FemMesh) -> Result<UnstructuredMesh, ConvertError> {
    let kind = kind_of(fem.element_type());
    let elements = (0..fem.n_elements())
        .map(|e| Element { kind, nodes: fem.element_nodes(ElemId(e)).to_vec() })
        .collect();
    Ok(UnstructuredMesh::from_elements(LengthUnit::Metre, fem.coords().to_vec(), elements, Vec::new(), Vec::new())?)
}

/// Neutral mesh -> farrer-park `Mesh` (coordinates scaled to metres).
///
/// # Errors
/// [`ConvertError::MixedElements`] unless every cell is the same typed
/// element kind (a `polyMesh` hex mesh qualifies once its cells were
/// recognised as Hex8); [`ConvertError::Solver`] if farrer-park refuses the
/// connectivity.
pub fn to_fem_mesh(mesh: &UnstructuredMesh) -> Result<FemMesh, ConvertError> {
    let first = mesh.cell_kind(0);
    let target = "a farrer-park Mesh";
    let Some(ty) = type_of(first) else {
        return Err(ConvertError::MixedElements { target, cell: 0, found: name_of(first), first: name_of(first) });
    };
    let mut connectivity = Vec::with_capacity(mesh.n_cells() * first.n_nodes().unwrap_or(0));
    for c in 0..mesh.n_cells() {
        let k = mesh.cell_kind(c);
        if k != first {
            return Err(ConvertError::MixedElements { target, cell: c, found: name_of(k), first: name_of(first) });
        }
        connectivity.extend_from_slice(mesh.cell_nodes(c));
    }
    let s = mesh.unit().metres_per_unit();
    let coords = mesh.points().iter().map(|p| [p[0] * s, p[1] * s, p[2] * s]).collect();
    FemMesh::new(ty, coords, connectivity).map_err(|e| ConvertError::Solver(e.to_string()))
}

/// One of farrer-park's structured generators (all lengths in metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FemGenerator {
    /// `farrer_park::mesh::box_hex8`: `[0, lx] x [0, ly] x [0, lz]`.
    BoxHex8 {
        /// Box sides \[m\].
        size: [f64; 3],
        /// Divisions per axis.
        divisions: [usize; 3],
    },
    /// `farrer_park::mesh::box_tet4`: the same box, six Kuhn tets per hex.
    BoxTet4 {
        /// Box sides \[m\].
        size: [f64; 3],
        /// Divisions per axis.
        divisions: [usize; 3],
    },
    /// `farrer_park::mesh::rectangle_quad4`: `[0, lx] x [0, ly]`.
    RectangleQuad4 {
        /// Rectangle sides \[m\].
        size: [f64; 2],
        /// Divisions per axis.
        divisions: [usize; 2],
    },
    /// `farrer_park::mesh::rectangle_tri6`.
    RectangleTri6 {
        /// Rectangle sides \[m\].
        size: [f64; 2],
        /// Divisions per axis.
        divisions: [usize; 2],
    },
    /// `farrer_park::mesh::quarter_annulus_quad4`: `r in [r_inner, r_outer]`,
    /// `theta in [0, pi/2]`.
    QuarterAnnulusQuad4 {
        /// Inner radius \[m\].
        r_inner: f64,
        /// Outer radius \[m\].
        r_outer: f64,
        /// Radial divisions.
        n_r: usize,
        /// Circumferential divisions.
        n_theta: usize,
    },
}

/// Run one of farrer-park's generators and return both its mesh and the
/// neutral mesh built from it.
///
/// # Errors
/// [`ConvertError::Solver`] with farrer-park's message for bad parameters,
/// or a neutral-mesh error.
pub fn generate(g: FemGenerator) -> Result<(UnstructuredMesh, FemMesh), ConvertError> {
    use farrer_park::mesh as fp;
    let fem = match g {
        FemGenerator::BoxHex8 { size, divisions: d } => fp::box_hex8(size[0], size[1], size[2], d[0], d[1], d[2]),
        FemGenerator::BoxTet4 { size, divisions: d } => fp::box_tet4(size[0], size[1], size[2], d[0], d[1], d[2]),
        FemGenerator::RectangleQuad4 { size, divisions: d } => fp::rectangle_quad4(size[0], size[1], d[0], d[1]),
        FemGenerator::RectangleTri6 { size, divisions: d } => fp::rectangle_tri6(size[0], size[1], d[0], d[1]),
        FemGenerator::QuarterAnnulusQuad4 { r_inner, r_outer, n_r, n_theta } => {
            fp::quarter_annulus_quad4(r_inner, r_outer, n_r, n_theta)
        }
    }
    .map_err(|e| ConvertError::Solver(e.to_string()))?;
    Ok((from_fem_mesh(&fem)?, fem))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// box_tet4 2 x 2 x 2 on a unit cube: 48 tets, unit total volume, and the
    /// round trip back to farrer-park returns the identical mesh.
    #[test]
    fn box_tet4_round_trip() {
        let (m, fem) = generate(FemGenerator::BoxTet4 { size: [1.0; 3], divisions: [2; 3] }).unwrap();
        assert_eq!(m.n_cells(), 48);
        assert!((m.total_volume() - 1.0).abs() < 1e-14);
        assert_eq!(to_fem_mesh(&m).unwrap(), fem);
    }

    /// A 2-D quarter annulus: the neutral area approaches pi (r_o^2 - r_i^2)/4
    /// from below (straight-sided quads under-fill the arc).
    #[test]
    fn quarter_annulus_area_is_below_the_arc() {
        let (m, _) =
            generate(FemGenerator::QuarterAnnulusQuad4 { r_inner: 1.0, r_outer: 2.0, n_r: 4, n_theta: 16 }).unwrap();
        assert_eq!(m.dim(), 2);
        let exact = std::f64::consts::PI * 3.0 / 4.0;
        assert!(m.total_volume() < exact && m.total_volume() > 0.99 * exact);
    }
}
