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

//! The curated public surface: everything needed to set up and run a
//! small-strain solid-mechanics problem, in one `use`.
//!
//! ```rust
//! use farrer_park::prelude::*;
//!
//! # fn main() -> farrer_park::error::Result<()> {
//! // A 4 x 4 plane-strain unit square of steel, stretched 0.01 % in x.
//! let mesh = unit_square_quad4(4)?.shared();
//! let dofs = DofMap::displacement(&mesh);
//! let material = Material::elastic(200.0e9, 0.3)?;
//! let mut system = System::new(mesh.clone(), material, BodyForce::None);
//!
//! let mut bcs = DirichletSet::new();
//! for n in mesh.nodes_where(|p| p[0].abs() < 1e-12) {
//!     bcs.fix(&dofs, n, 0, 0.0);
//! }
//! for n in mesh.nodes_where(|p| (p[0] - 1.0).abs() < 1e-12) {
//!     bcs.fix(&dofs, n, 0, 1.0e-4);
//! }
//! for n in mesh.nodes_where(|p| p[1].abs() < 1e-12) {
//!     bcs.fix(&dofs, n, 1, 0.0);
//! }
//!
//! let forces = vec![0.0; system.n_dofs()];
//! let (u, report) = solve_linear_elastic(
//!     &mut system, &bcs, &forces, LinearSolverSettings::default(),
//! )?;
//!
//! // Two solves: one lands on the answer, one confirms both convergence
//! // criteria. See `solver::NewtonSettings` for why.
//! assert_eq!(report.steps[0].iterations, 2);
//! assert!((u[2 * (mesh.n_nodes() - 1)] - 1.0e-4).abs() < 1e-12);
//! # Ok(())
//! # }
//! ```
//!
//! # What is deliberately NOT re-exported
//!
//! The low-level pieces a user assembling their own element loop would need —
//! [`crate::element::map_gradients`], [`crate::quadrature`]'s individual rules,
//! [`crate::sparse::CsrMatrix`]'s internals. They are all public in their own
//! modules; keeping them out of the prelude is what stops it from becoming a
//! second, undocumented copy of the crate's API.

pub use crate::assembly::{interpolate, Assembled, BodyForce, System};
pub use crate::bc::{accumulate_pressure_2d, DirichletMethod, DirichletSet, Traction};
pub use crate::dof::{DofId, DofMap};
pub use crate::element::{ElementType, FacetType};
pub use crate::error::{FemError, Result};
pub use crate::material::{
    BulkModulus, HardeningModulus, J2LinearHardening, LinearElastic, Material, MaterialState,
    PoissonRatio, ShearModulus, StressUpdate, YieldStress, YoungsModulus,
};
pub use crate::mesh::{
    box_hex8, box_tet4, quarter_annulus_inner_facets, quarter_annulus_quad4, rectangle_quad4,
    rectangle_right_edge_facets, rectangle_tri3, rectangle_tri6, tri3_to_tri6,
    tri6_edge_facets_at_x, unit_cube_hex8, unit_cube_tet4, unit_square_quad4, unit_square_tri3,
    unit_square_tri6, ElemId, Facet, Mesh, NodeId,
};
pub use crate::solver::{
    solve_linear, solve_linear_elastic, solve_nonlinear, KrylovMethod, LinearSolverSettings,
    LoadStepReport, NewtonReport, NewtonSettings, PreconditionerChoice,
};
pub use crate::tensor::{Tensor4, Voigt6};
