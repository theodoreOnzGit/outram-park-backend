// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Layer addition — Phase 3 of `snappyHexMesh` (**scaffolded, not yet
//! implemented**).
//!
//! The final phase inserts graded prismatic ("boundary layer") cells between the
//! snapped surface and the interior mesh, to resolve near-wall gradients.
//! OpenFOAM's algorithm (`snappyLayerDriver`) is:
//!
//! 1. **Pick layer patches.** Choose which boundary patches get layers and how
//!    many (`nSurfaceLayers`).
//! 2. **Extrude the surface inward.** Duplicate the patch points and displace
//!    the mesh interior along the point normals to open a gap of total layer
//!    thickness, filling it with `nSurfaceLayers` new prism cells.
//! 3. **Grade the thickness.** Size the layers by `expansionRatio` and either
//!    `finalLayerThickness` or `firstLayerThickness`, so cell height grows
//!    geometrically away from the wall. For a ratio `r` and `n` layers the
//!    thicknesses are `t, t·r, t·r², …`; the total is `t·(rⁿ − 1)/(r − 1)`.
//! 4. **Quality-limited collapse.** Where adding a full layer would violate the
//!    `meshQualityControls` (thin corners, high non-orthogonality), locally
//!    reduce the layer count or collapse the layer — the robustness core of the
//!    real utility.
//!
//! The geometric grading (step 3) is pure arithmetic and is provided as
//! [`layer_thicknesses`] so it can be unit-tested independently. The topological
//! extrusion (steps 1, 2, 4) needs the point/face-vertex mesh representation and
//! is not yet implemented; [`add_layers`] is an honest stub.

use crate::snappy_hex_mesh::castellation::CastellatedMesh;
use crate::MeshError;

/// Controls for the layer-addition phase (subset of `addLayersControls`).
#[derive(Debug, Clone)]
pub struct LayerControls {
    /// Number of prism layers to add at the wall.
    pub n_surface_layers: usize,
    /// Geometric expansion ratio between successive layers (`> 0`, usually
    /// `> 1` so cells grow away from the wall).
    pub expansion_ratio: f64,
    /// Thickness of the layer nearest the wall [m].
    pub first_layer_thickness: f64,
}

impl Default for LayerControls {
    fn default() -> Self {
        Self {
            n_surface_layers: 3,
            expansion_ratio: 1.2,
            first_layer_thickness: 1e-3,
        }
    }
}

/// Geometric layer thicknesses `[t, t·r, t·r², …]` [m] for `n` layers with
/// first-layer thickness `t` and expansion ratio `r`.
///
/// This is the grading arithmetic of Phase 3 in isolation (fully testable). The
/// returned vector has length `controls.n_surface_layers`; the total boundary-
/// layer thickness is its sum, `t·(rⁿ − 1)/(r − 1)` for `r ≠ 1`.
pub fn layer_thicknesses(controls: &LayerControls) -> Vec<f64> {
    let mut out = Vec::with_capacity(controls.n_surface_layers);
    let mut t = controls.first_layer_thickness;
    for _ in 0..controls.n_surface_layers {
        out.push(t);
        t *= controls.expansion_ratio;
    }
    out
}

/// Total boundary-layer thickness [m] — the sum of [`layer_thicknesses`].
pub fn total_layer_thickness(controls: &LayerControls) -> f64 {
    layer_thicknesses(controls).iter().sum()
}

/// Insert graded prism layers at the wall (**not yet implemented**).
///
/// Returns [`MeshError::NotImplemented`]. The grading maths is available via
/// [`layer_thicknesses`]; the surface extrusion and quality-limited collapse
/// (which need the point/face-vertex topology) are the remaining work.
pub fn add_layers(
    _mesh: &CastellatedMesh,
    _controls: &LayerControls,
) -> Result<CastellatedMesh, MeshError> {
    Err(MeshError::NotImplemented(
        "snappyHexMesh layer-addition phase: surface extrusion and quality-limited \
         layer collapse are scaffolded but not implemented (see layers module docs; \
         layer_thicknesses provides the grading arithmetic)"
            .to_string(),
    ))
}
