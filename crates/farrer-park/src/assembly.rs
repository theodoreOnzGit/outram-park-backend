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

//! Weak-form assembly: the element loop, the quadrature loop, the
//! strain-displacement operator, and the internal force and tangent they
//! produce.
//!
//! # What belongs in this module
//!
//! The discrete momentum balance
//!
//! `integral over V of B^T sigma dV = integral over V of N^T b dV + boundary tractions`
//!
//! assembled as a residual and a tangent: [`System`] owns the mesh, the
//! material and the per-quadrature-point state, and
//! [`System::assemble`] produces the internal force vector and the stiffness
//! matrix at a given displacement.
//!
//! # What does NOT belong here
//!
//! Boundary conditions ([`crate::bc`]), the Newton loop and load stepping
//! ([`crate::solver`]), and the constitutive integration itself
//! ([`crate::material`]). This module supplies strains to the material and
//! scatters what comes back.
//!
//! # Small strain, total Lagrangian on the reference configuration
//!
//! Everything is linearised about the undeformed geometry:
//! `eps = (grad u + grad u^T) / 2`, integration on the reference volume, no
//! geometric stiffness. That is the correct model up to strains of a percent or
//! so and rotations of a few degrees, and outside that it is simply wrong — not
//! inaccurate, wrong, because a rigid rotation would generate stress. Finite
//! strain is deliberately out of scope for this pass.
//!
//! # Two-dimensional analysis: plane strain **or** plane stress
//!
//! [`crate::material::PlaneCondition`] selects, it is carried on [`System`]
//! through [`SystemOptions`], and **plane strain is the default**.
//!
//! Plane strain is a kinematic condition: `eps_zz = gamma_yz = gamma_xz = 0`,
//! which the strain-displacement operator imposes by writing zeros into those
//! three Voigt slots, after which the ordinary three-dimensional constitutive
//! law gives `sigma_zz = nu (sigma_xx + sigma_yy)` with no special case.
//!
//! Plane stress is a *constitutive* condition, `sigma_zz = 0`, so it is not a
//! matter of zeroing different slots and it is not handled here at all: the
//! B-matrix is unchanged and [`crate::material`] condenses `eps_zz` out of the
//! law (a nested scalar Newton inside the return map, for J2). It is rejected
//! on a three-dimensional mesh, where it is meaningless.
//!
//! # Element formulation: full integration (default) or B-bar
//!
//! [`Formulation`] selects, and **full integration is the default**. B-bar
//! (mean dilatation) replaces the volumetric part of the strain-displacement
//! operator with its element average, which is the standard cure for
//! **volumetric locking** — the over-stiff response of a low-order element
//! whose material is nearly incompressible, either elastically as `nu -> 0.5`
//! or plastically, because J2 flow preserves volume. It does **nothing** for
//! shear locking, which is a different mechanism; see [`Formulation`].
//!
//! # Units
//!
//! Displacements in metres, strains dimensionless, stresses in pascals, the
//! internal force vector in newtons, the stiffness matrix in newton per metre,
//! body force in newton per cubic metre.

use std::sync::Arc;

use crate::dof::{DofMap, SparsityPattern};
use crate::element::{map_gradients, map_point, ElementType, MAX_ELEM_NODES};
use crate::error::{FemError, Result};
use crate::material::{Material, MaterialState, PlaneCondition, StressUpdate};
use crate::mesh::{ElemId, Mesh};
use crate::quadrature::{default_rule, error_rule, QuadraturePoint};
use crate::sparse::CsrMatrix;
use crate::tensor::{Tensor4, Voigt6};

/// Maximum element degrees of freedom: eight nodes times three components.
pub const MAX_ELEM_DOFS: usize = MAX_ELEM_NODES * 3;

/// The strain-displacement operator `B` of one element at one quadrature point.
///
/// `eps_V = B u_e`, with `u_e` the element nodal displacements in metres, in
/// node-major order (all components of node 0, then node 1, ...), and `eps_V`
/// the six-component engineering-shear strain of [`crate::tensor`].
///
/// # Units
///
/// Entries are in reciprocal metres.
///
/// # Layout
///
/// Row-major `6 x n_dofs`, with `n_dofs = n_nodes * dim`. Rows 2, 3 and 4
/// (`zz`, `yz`, `xz`) are identically zero for a 2-D element as
/// [`BMatrix::from_gradients`] builds it — that is the plane-strain
/// constraint, applied in the one place it belongs.
/// [`BMatrix::apply_mean_dilatation`] deliberately breaks the `zz` zero; see
/// its documentation for why that is correct and not a lapse.
#[derive(Debug, Clone, Copy)]
pub struct BMatrix {
    /// `6 x MAX_ELEM_DOFS` row-major, reciprocal metres. Columns beyond
    /// `n_dofs` are zero.
    pub b: [[f64; MAX_ELEM_DOFS]; 6],
    /// Number of meaningful columns: `n_nodes * dim`.
    pub n_dofs: usize,
}

impl BMatrix {
    /// Build `B` from the physical shape-function gradients `dN_a/dx_i`
    /// \[1/m\].
    ///
    /// # Arguments
    ///
    /// - `grad` — `[a][i] = dN_a/dx_i`, as returned by
    ///   [`crate::element::map_gradients`].
    /// - `n_nodes` — nodes on the element.
    /// - `dim` — 2 (plane strain) or 3.
    #[must_use]
    pub fn from_gradients(grad: &[[f64; 3]; MAX_ELEM_NODES], n_nodes: usize, dim: usize) -> Self {
        let mut b = [[0.0_f64; MAX_ELEM_DOFS]; 6];
        for a in 0..n_nodes {
            let (gx, gy, gz) = (grad[a][0], grad[a][1], grad[a][2]);
            let base = a * dim;
            if dim == 2 {
                // eps_xx, eps_yy, gamma_xy; zz / yz / xz remain zero.
                b[0][base] = gx;
                b[1][base + 1] = gy;
                b[5][base] = gy;
                b[5][base + 1] = gx;
            } else {
                b[0][base] = gx;
                b[1][base + 1] = gy;
                b[2][base + 2] = gz;
                b[3][base + 1] = gz; // gamma_yz = du_y/dz + du_z/dy
                b[3][base + 2] = gy;
                b[4][base] = gz; // gamma_xz = du_x/dz + du_z/dx
                b[4][base + 2] = gx;
                b[5][base] = gy; // gamma_xy = du_x/dy + du_y/dx
                b[5][base + 1] = gx;
            }
        }
        BMatrix {
            b,
            n_dofs: n_nodes * dim,
        }
    }

    /// The **dilatational row** of `B` at this point: `B_v[k] = d N_a / d x_i`
    /// for the degree of freedom `k = a * dim + i`, so that
    /// `B_v . u_e = div u = tr(eps)`.
    ///
    /// It is the sum of the first three rows of `B`, which is exactly the trace
    /// operator in this Voigt convention. Used to form the element-mean
    /// dilatation of the B-bar formulation.
    ///
    /// # Units
    ///
    /// Reciprocal metres. Entries beyond `n_dofs` are zero.
    #[must_use]
    pub fn dilatational_row(&self) -> [f64; MAX_ELEM_DOFS] {
        let mut v = [0.0_f64; MAX_ELEM_DOFS];
        for (k, vk) in v.iter_mut().enumerate().take(self.n_dofs) {
            *vk = self.b[0][k] + self.b[1][k] + self.b[2][k];
        }
        v
    }

    /// Replace the volumetric part of this operator with an element-averaged
    /// one — the **B-bar (mean dilatation)** modification.
    ///
    /// # What it does
    ///
    /// Split the strain into deviatoric and volumetric parts,
    /// `eps = dev(eps) + (1/3) tr(eps) I`. B-bar keeps `dev(eps)` pointwise and
    /// replaces `tr(eps)` by the element average `theta_bar`:
    ///
    /// `B_bar = B + (1/3) (B_v_bar - B_v) (x) I`
    ///
    /// which in this Voigt convention means adding
    /// `(B_v_bar[k] - B_v[k]) / 3` to entries `[0][k]`, `[1][k]` and `[2][k]`
    /// and leaving the three shear rows alone.
    ///
    /// # Arguments
    ///
    /// - `mean_dilatation` — `B_v_bar[k] = (1 / V_e) integral of B_v[k] dV`
    ///   over the element, in reciprocal metres, the same layout as
    ///   [`BMatrix::dilatational_row`]. Only the first `n_dofs` entries are
    ///   read.
    ///
    /// # Why the `zz` row stops being zero in two dimensions, and why that is
    /// right
    ///
    /// Under plane strain `eps_zz = 0` but `tr(eps) = eps_xx + eps_yy` is not,
    /// so the volumetric part of the strain tensor has a `zz` component and the
    /// modification writes `(theta_bar - theta) / 3` there. That is the
    /// standard plane-strain B-bar element, not a leak: the deviatoric strain
    /// is untouched and only the element-mean dilatation is substituted, which
    /// is precisely the constraint relaxation the method exists to perform.
    ///
    /// # Where it is a no-op, and therefore useless
    ///
    /// On a constant-strain element (Tri3, Tet4) the gradients do not vary
    /// within the element, so `B_v_bar = B_v` identically and this function
    /// changes nothing. Those elements lock volumetrically and B-bar does not
    /// help them — the fix there is a different element, not a different
    /// integration of this one. The same holds on any element under a strain
    /// field that is already constant, which is why the patch test is
    /// unaffected.
    pub fn apply_mean_dilatation(&mut self, mean_dilatation: &[f64]) {
        for k in 0..self.n_dofs {
            let b_v = self.b[0][k] + self.b[1][k] + self.b[2][k];
            let delta = (mean_dilatation[k] - b_v) / 3.0;
            self.b[0][k] += delta;
            self.b[1][k] += delta;
            self.b[2][k] += delta;
        }
    }

    /// Strain at this point from the element nodal displacements \[m\]:
    /// `eps = B u_e` \[-\].
    #[must_use]
    pub fn strain(&self, ue: &[f64]) -> Voigt6 {
        let mut e = [0.0_f64; 6];
        for (i, ei) in e.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in 0..self.n_dofs {
                s += self.b[i][k] * ue[k];
            }
            *ei = s;
        }
        Voigt6(e)
    }

    /// Accumulate `w B^T sigma` into the element internal force `fe` \[N\].
    ///
    /// `w` is the physical integration weight `weight * det J` in cubic metres
    /// (2-D: square metres times unit thickness).
    pub fn accumulate_internal_force(&self, sigma: &Voigt6, w: f64, fe: &mut [f64]) {
        for k in 0..self.n_dofs {
            let mut s = 0.0;
            for i in 0..6 {
                s += self.b[i][k] * sigma.0[i];
            }
            fe[k] += w * s;
        }
    }

    /// Accumulate `w B^T D B` into the element stiffness `ke` \[N/m\],
    /// row-major of side `n_dofs`.
    pub fn accumulate_stiffness(&self, d: &Tensor4, w: f64, ke: &mut [f64]) {
        let n = self.n_dofs;
        // db[i][k] = sum_j D[i][j] B[j][k]
        let mut db = [[0.0_f64; MAX_ELEM_DOFS]; 6];
        for i in 0..6 {
            for k in 0..n {
                let mut s = 0.0;
                for j in 0..6 {
                    s += d.0[i][j] * self.b[j][k];
                }
                db[i][k] = s;
            }
        }
        for a in 0..n {
            for bcol in 0..n {
                let mut s = 0.0;
                for i in 0..6 {
                    s += self.b[i][a] * db[i][bcol];
                }
                ke[a * n + bcol] += w * s;
            }
        }
    }
}

/// How the strain-displacement operator is built — the element formulation.
///
/// A closed set, dispatched by `match`, not a boolean and not a hidden global:
/// the choice changes the element's answer, so it should be visible at the call
/// site.
///
/// # Units
///
/// Dimensionless — this is a selector, not a physical quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Formulation {
    /// **Full integration** — the default, and what every verification case in
    /// this crate outside `tests/locking.rs` was run with.
    ///
    /// `B` is used exactly as [`BMatrix::from_gradients`] builds it, evaluated
    /// at each point of [`crate::quadrature::default_rule`]. Correct, variationally
    /// clean, and the right choice for a compressible material.
    ///
    /// It **locks** in two distinct ways, both measured in
    /// `docs/verification.md`:
    ///
    /// - *volumetrically*, when the material is nearly incompressible
    ///   (`nu -> 0.5`, or any fully plastic zone, since J2 flow preserves
    ///   volume). Cured by [`Formulation::BBar`].
    /// - *in shear*, when a low-order element is bent. **Not** cured by B-bar;
    ///   the fix is a higher-order element or an incompatible-modes /
    ///   enhanced-strain formulation, neither of which this crate has.
    #[default]
    FullIntegration,
    /// **B-bar (mean dilatation)** — the volumetric part of `B` is replaced by
    /// its element average, following Hughes' generalisation of selective
    /// reduced integration.
    ///
    /// Reference: T. J. R. Hughes, "Generalization of selective integration
    /// procedures to anisotropic and nonlinear media", *International Journal
    /// for Numerical Methods in Engineering* **15**(9), 1413-1418 (1980); and
    /// T. J. R. Hughes, *The Finite Element Method*, Dover 2000, section 4.5.
    ///
    /// The mechanics live in [`BMatrix::apply_mean_dilatation`]. Three
    /// properties matter and are each verified:
    ///
    /// - It is **exactly consistent**: the internal force uses `B_bar` and the
    ///   tangent uses `B_bar^T D B_bar` with `D` evaluated at
    ///   `eps = B_bar u_e`, so the tangent is still the exact derivative of the
    ///   internal force and Newton still converges quadratically.
    /// - It **passes the patch test**, because a constant strain field has
    ///   `theta_bar = theta` and the modification vanishes.
    /// - It is a **no-op on Tri3 and Tet4**, whose gradients are already
    ///   constant within the element. Selecting it there is allowed and
    ///   harmless, but it cures nothing.
    ///
    /// Not combinable with [`crate::material::PlaneCondition::PlaneStress`],
    /// which has no volumetric constraint to relax — see
    /// [`SystemOptions::validate`].
    BBar,
}

impl Formulation {
    /// A short name for diagnostics and table headings.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Formulation::FullIntegration => "full integration",
            Formulation::BBar => "B-bar",
        }
    }
}

/// The modelling choices a [`System`] carries beyond its mesh, material and
/// body force.
///
/// Both fields default to the conservative option — full integration, plane
/// strain — so `SystemOptions::default()` reproduces exactly what
/// [`System::new`] builds.
///
/// # Units
///
/// Dimensionless; both fields are selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SystemOptions {
    /// Element formulation. Default [`Formulation::FullIntegration`].
    pub formulation: Formulation,
    /// Out-of-plane condition for a two-dimensional analysis. Default
    /// [`PlaneCondition::PlaneStrain`], which is also the only valid setting on
    /// a three-dimensional mesh.
    pub plane_condition: PlaneCondition,
}

impl SystemOptions {
    /// Check the combination is one this crate implements and has verified.
    ///
    /// # Arguments
    ///
    /// - `dim` — the mesh dimension, 2 or 3.
    ///
    /// # Errors
    ///
    /// [`FemError::BoundaryCondition`] carrying the reason, for either of the
    /// two rejected combinations:
    ///
    /// - **Plane stress on a three-dimensional mesh.** There is no out-of-plane
    ///   direction to condense; the request is meaningless rather than merely
    ///   unimplemented.
    /// - **B-bar with plane stress.** Plane stress has no volumetric
    ///   constraint to relax: the out-of-plane strain is free, so an
    ///   incompressible material accommodates the volume change through
    ///   `eps_zz` without constraining the in-plane displacement field at all.
    ///   B-bar therefore has nothing to cure there, and the interaction between
    ///   the mean-dilatation modification (which writes into the `zz` row) and
    ///   the condensation (which solves for `eps_zz`) is not a combination this
    ///   crate has verified. Refusing it is deliberate.
    pub fn validate(&self, dim: usize) -> Result<()> {
        if self.plane_condition == PlaneCondition::PlaneStress && dim == 3 {
            return Err(FemError::BoundaryCondition(
                "plane stress is a two-dimensional idealisation and has no meaning on a \
                 three-dimensional mesh; use PlaneCondition::PlaneStrain, which is a no-op \
                 there"
                    .into(),
            ));
        }
        if self.formulation == Formulation::BBar
            && self.plane_condition == PlaneCondition::PlaneStress
        {
            return Err(FemError::BoundaryCondition(
                "B-bar is not combinable with plane stress: plane stress has no volumetric \
                 constraint to relax, because the out-of-plane strain is free to accommodate \
                 any volume change, so a nearly incompressible material does not lock there \
                 in the first place"
                    .into(),
            ));
        }
        Ok(())
    }
}

/// A body force field, dispatched by enum rather than a boxed closure.
///
/// # Units
///
/// Newton per cubic metre (2-D: newton per square metre through unit
/// thickness).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyForce {
    /// No body force.
    None,
    /// A spatially uniform force per unit volume `[b_x, b_y, b_z]`
    /// \[N/m^3\] — gravity, for instance.
    Uniform([f64; 3]),
    /// The manufactured body force of the verification study,
    ///
    /// `f_x = f_y = pi^2 [(lambda + 3 mu) sin(pi x) sin(pi y)
    ///                    - (lambda + mu) cos(pi x) cos(pi y)]`,
    ///
    /// which is `-div sigma` for the exact plane-strain displacement field
    /// `u_x = u_y = sin(pi x) sin(pi y)` on the unit square. It lives here
    /// rather than in the test file because [`BodyForce`] is an enum and a
    /// closure variant would be a trait object.
    ///
    /// Carries the Lame constants `(lambda, mu)` in pascals.
    ManufacturedSine(f64, f64),
    /// The **three-dimensional** manufactured body force, for the exact
    /// displacement field
    ///
    /// `u_x = u_y = u_z = sin(pi x) sin(pi y) sin(pi z)` \[m\]
    ///
    /// on the unit cube. Writing `a = sin(pi x)`, `A = cos(pi x)` and likewise
    /// `b, B` for `y` and `c, C` for `z`, substituting into
    /// `div sigma = (lambda + mu) grad(div u) + mu laplacian(u)` gives
    /// `f = -div sigma` as
    ///
    /// `f_x = pi^2 [(lambda + 4 mu) a b c - (lambda + mu) (A B c + A b C)]`
    ///
    /// `f_y = pi^2 [(lambda + 4 mu) a b c - (lambda + mu) (A B c + a B C)]`
    ///
    /// `f_z = pi^2 [(lambda + 4 mu) a b c - (lambda + mu) (A b C + a B C)]`
    ///
    /// in newton per cubic metre. The field does **not** vanish on the whole
    /// boundary of the unit cube — it vanishes on all six faces, since each
    /// face sets one of `a, b, c` to zero — so homogeneous Dirichlet data is
    /// exact here as well.
    ///
    /// Its purpose is the nearly-incompressible convergence study on Hex8,
    /// where the two-dimensional [`BodyForce::ManufacturedSine`] cannot reach.
    ///
    /// Carries the Lame constants `(lambda, mu)` in pascals.
    ManufacturedSine3d(f64, f64),
}

impl BodyForce {
    /// Evaluate the body force at a physical point `x` \[m\], returning
    /// \[N/m^3\].
    #[must_use]
    pub fn at(&self, x: [f64; 3]) -> [f64; 3] {
        match self {
            BodyForce::None => [0.0; 3],
            BodyForce::Uniform(b) => *b,
            BodyForce::ManufacturedSine(lambda, mu) => {
                let pi = std::f64::consts::PI;
                let a = (pi * x[0]).sin() * (pi * x[1]).sin();
                let c = (pi * x[0]).cos() * (pi * x[1]).cos();
                let f = pi * pi * ((lambda + 3.0 * mu) * a - (lambda + mu) * c);
                [f, f, 0.0]
            }
            BodyForce::ManufacturedSine3d(lambda, mu) => {
                let pi = std::f64::consts::PI;
                let (sa, ca) = ((pi * x[0]).sin(), (pi * x[0]).cos());
                let (sb, cb) = ((pi * x[1]).sin(), (pi * x[1]).cos());
                let (sc, cc) = ((pi * x[2]).sin(), (pi * x[2]).cos());
                let abc = sa * sb * sc;
                let d_xy = ca * cb * sc;
                let d_xz = ca * sb * cc;
                let d_yz = sa * cb * cc;
                let common = (lambda + 4.0 * mu) * abc;
                let k = lambda + mu;
                [
                    pi * pi * (common - k * (d_xy + d_xz)),
                    pi * pi * (common - k * (d_xy + d_yz)),
                    pi * pi * (common - k * (d_xz + d_yz)),
                ]
            }
        }
    }

    /// The exact displacement field \[m\] this body force was manufactured
    /// from, and its gradient `du_i/dx_j` \[-\].
    ///
    /// Returns `None` for the variants that are not manufactured solutions.
    /// Used by the convergence study to form the `L2` and `H1` errors.
    #[must_use]
    pub fn manufactured_solution(&self, x: [f64; 3]) -> Option<([f64; 3], [[f64; 3]; 3])> {
        match self {
            BodyForce::ManufacturedSine(_, _) => {
                let pi = std::f64::consts::PI;
                let u = (pi * x[0]).sin() * (pi * x[1]).sin();
                let dudx = pi * (pi * x[0]).cos() * (pi * x[1]).sin();
                let dudy = pi * (pi * x[0]).sin() * (pi * x[1]).cos();
                let mut g = [[0.0; 3]; 3];
                g[0][0] = dudx;
                g[0][1] = dudy;
                g[1][0] = dudx;
                g[1][1] = dudy;
                Some(([u, u, 0.0], g))
            }
            BodyForce::ManufacturedSine3d(_, _) => {
                let pi = std::f64::consts::PI;
                let (sa, ca) = ((pi * x[0]).sin(), (pi * x[0]).cos());
                let (sb, cb) = ((pi * x[1]).sin(), (pi * x[1]).cos());
                let (sc, cc) = ((pi * x[2]).sin(), (pi * x[2]).cos());
                let u = sa * sb * sc;
                let d = [pi * ca * sb * sc, pi * sa * cb * sc, pi * sa * sb * cc];
                let mut g = [[0.0; 3]; 3];
                for row in g.iter_mut() {
                    row.copy_from_slice(&d);
                }
                Some(([u, u, u], g))
            }
            _ => None,
        }
    }
}

/// The assembled finite-element problem: mesh, degrees of freedom, material,
/// quadrature-point history, and the matrix they fill.
///
/// The mesh is shared with `Arc<Mesh>` and never mutated, per the workspace
/// rule for read-only data. There are no lifetime parameters: everything else
/// is owned by value.
#[derive(Debug, Clone)]
pub struct System {
    mesh: Arc<Mesh>,
    dofs: DofMap,
    material: Material,
    body_force: BodyForce,
    rule: Vec<QuadraturePoint>,
    /// Committed history at every quadrature point, indexed
    /// `element * n_quadrature + point`.
    state: Vec<MaterialState>,
    /// Stress at every quadrature point at the last assembled displacement
    /// \[Pa\], same indexing. Post-processing reads it; assembly rewrites it.
    stress: Vec<Voigt6>,
    /// Trial history from the last assembly, not yet committed.
    trial_state: Vec<MaterialState>,
    pattern: SparsityPattern,
    options: SystemOptions,
}

/// What one assembly produced.
#[derive(Debug, Clone)]
pub struct Assembled {
    /// Internal force vector `f_int` \[N\], length `n_dofs`.
    pub internal_force: Vec<f64>,
    /// External force from the body force \[N\], length `n_dofs`. Tractions are
    /// added by [`crate::bc`].
    pub body_force: Vec<f64>,
    /// How many quadrature points flowed plastically in this assembly.
    pub n_yielding: usize,
}

impl System {
    /// Build a system on a mesh with one material and one body force, using
    /// the **default** modelling options: full integration, plane strain.
    ///
    /// Uses [`crate::quadrature::default_rule`] for the element type; the
    /// per-point history is initialised pristine. Equivalent to
    /// [`System::with_options`] with [`SystemOptions::default`], which cannot
    /// fail.
    #[must_use]
    pub fn new(mesh: Arc<Mesh>, material: Material, body_force: BodyForce) -> Self {
        Self::build(mesh, material, body_force, SystemOptions::default())
    }

    /// Build a system with explicit modelling options — the element
    /// formulation and the two-dimensional out-of-plane condition.
    ///
    /// # Errors
    ///
    /// Whatever [`SystemOptions::validate`] rejects: plane stress on a
    /// three-dimensional mesh, or B-bar combined with plane stress.
    pub fn with_options(
        mesh: Arc<Mesh>,
        material: Material,
        body_force: BodyForce,
        options: SystemOptions,
    ) -> Result<Self> {
        options.validate(mesh.dim())?;
        Ok(Self::build(mesh, material, body_force, options))
    }

    fn build(
        mesh: Arc<Mesh>,
        material: Material,
        body_force: BodyForce,
        options: SystemOptions,
    ) -> Self {
        let dofs = DofMap::displacement(&mesh);
        let rule = default_rule(mesh.element_type());
        let n_pts = mesh.n_elements() * rule.len();
        let pattern = SparsityPattern::from_mesh(&mesh, &dofs);
        Self {
            mesh,
            dofs,
            material,
            body_force,
            rule,
            state: vec![MaterialState::pristine(); n_pts],
            stress: vec![Voigt6::ZERO; n_pts],
            trial_state: vec![MaterialState::pristine(); n_pts],
            pattern,
            options,
        }
    }

    /// The modelling options this system was built with.
    #[must_use]
    pub fn options(&self) -> SystemOptions {
        self.options
    }

    /// The element formulation in use.
    #[must_use]
    pub fn formulation(&self) -> Formulation {
        self.options.formulation
    }

    /// The two-dimensional out-of-plane condition in use.
    #[must_use]
    pub fn plane_condition(&self) -> PlaneCondition {
        self.options.plane_condition
    }

    /// The mesh, shared.
    #[must_use]
    pub fn mesh(&self) -> Arc<Mesh> {
        Arc::clone(&self.mesh)
    }

    /// The degree-of-freedom map.
    #[must_use]
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }

    /// The material law.
    #[must_use]
    pub fn material(&self) -> Material {
        self.material
    }

    /// Total number of degrees of freedom (dimensionless count).
    #[must_use]
    pub fn n_dofs(&self) -> usize {
        self.dofs.n_dofs()
    }

    /// Quadrature points per element (dimensionless count).
    #[must_use]
    pub fn n_quadrature_points(&self) -> usize {
        self.rule.len()
    }

    /// Stress at every quadrature point from the last assembly \[Pa\], indexed
    /// `element * n_quadrature_points() + point`.
    #[must_use]
    pub fn quadrature_stress(&self) -> &[Voigt6] {
        &self.stress
    }

    /// Committed history at every quadrature point.
    #[must_use]
    pub fn quadrature_state(&self) -> &[MaterialState] {
        &self.state
    }

    /// Physical coordinates \[m\] of every quadrature point, in the same order
    /// as [`quadrature_stress`](Self::quadrature_stress).
    #[must_use]
    pub fn quadrature_coordinates(&self) -> Vec<[f64; 3]> {
        let et = self.mesh.element_type();
        let mut ec = [[0.0; 3]; MAX_ELEM_NODES];
        let mut out = Vec::with_capacity(self.stress.len());
        for e in 0..self.mesh.n_elements() {
            self.mesh.element_coords(ElemId(e), &mut ec);
            for q in &self.rule {
                out.push(map_point(et, &ec, q.xi));
            }
        }
        out
    }

    /// A fresh matrix on this system's sparsity pattern.
    #[must_use]
    pub fn new_matrix(&self) -> CsrMatrix {
        CsrMatrix::from_pattern(&self.pattern)
    }

    /// Commit the trial history produced by the last [`assemble`](Self::assemble)
    /// call.
    ///
    /// Call this **once** per converged load step, never inside the Newton
    /// loop: committing mid-iteration would let a rejected iterate leave
    /// permanent plastic strain behind.
    pub fn commit(&mut self) {
        self.state.copy_from_slice(&self.trial_state);
    }

    /// Assemble the internal force, the body force and (optionally) the
    /// tangent stiffness at displacement `u`.
    ///
    /// # Arguments
    ///
    /// - `u` — nodal displacements \[m\], length [`n_dofs`](Self::n_dofs).
    /// - `matrix` — `Some(k)` to assemble the tangent into `k` (which is zeroed
    ///   first); `None` to compute forces only, which is what a residual-only
    ///   evaluation inside a line search would want.
    ///
    /// # Returns
    ///
    /// The internal and body force vectors \[N\], and how many quadrature
    /// points yielded.
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] on a wrong-length `u`,
    /// [`FemError::DegenerateElement`] (with the real element and point index)
    /// for a non-positive Jacobian, and whatever the constitutive update
    /// returns.
    pub fn assemble(&mut self, u: &[f64], matrix: Option<&mut CsrMatrix>) -> Result<Assembled> {
        if u.len() != self.n_dofs() {
            return Err(FemError::LengthMismatch {
                context: "System::assemble: displacement vector",
                expected: self.n_dofs(),
                actual: u.len(),
            });
        }
        let et = self.mesh.element_type();
        let nn = et.n_nodes();
        let dim = et.dim();
        let ned = nn * dim;
        let nq = self.rule.len();

        let mut f_int = vec![0.0; self.n_dofs()];
        let mut f_body = vec![0.0; self.n_dofs()];
        let mut n_yield = 0usize;

        let mut ec = [[0.0_f64; 3]; MAX_ELEM_NODES];
        let mut ed = vec![0usize; ned];
        let mut ue = vec![0.0_f64; ned];
        let mut fe = vec![0.0_f64; ned];
        let mut fb = vec![0.0_f64; ned];
        let mut ke = vec![0.0_f64; ned * ned];

        let mut mat = matrix;
        if let Some(k) = mat.as_deref_mut() {
            k.zero();
        }

        // Isoparametric mapping cached per quadrature point, because the B-bar
        // formulation needs a pre-pass over the whole element (to form the mean
        // dilatation) before any point can be assembled, and mapping twice
        // would be both slower and a second place for the two passes to
        // disagree.
        let mut mapped: Vec<crate::element::MappedPoint> = Vec::with_capacity(nq);
        let mut mean_dilatation = [0.0_f64; MAX_ELEM_DOFS];
        let bbar = self.options.formulation == Formulation::BBar;
        let plane = self.options.plane_condition;

        for e in 0..self.mesh.n_elements() {
            self.mesh.element_coords(ElemId(e), &mut ec);
            self.dofs.element_dofs(&self.mesh, ElemId(e), &mut ed);
            for (i, &g) in ed.iter().enumerate() {
                ue[i] = u[g];
            }
            fe.iter_mut().for_each(|v| *v = 0.0);
            fb.iter_mut().for_each(|v| *v = 0.0);
            ke.iter_mut().for_each(|v| *v = 0.0);

            mapped.clear();
            for (qi, q) in self.rule.iter().enumerate() {
                mapped.push(map_gradients(et, &ec, q.xi).map_err(|err| match err {
                    FemError::DegenerateElement { det_j, .. } => FemError::DegenerateElement {
                        element: e,
                        point: qi,
                        det_j,
                    },
                    other => other,
                })?);
            }

            // B-bar pre-pass: the element-mean dilatational row,
            // `B_v_bar = (1 / V_e) integral of B_v dV`, in reciprocal metres.
            if bbar {
                mean_dilatation[..ned].iter_mut().for_each(|v| *v = 0.0);
                let mut volume = 0.0_f64;
                for (qi, q) in self.rule.iter().enumerate() {
                    let w = q.weight * mapped[qi].det_j;
                    volume += w;
                    let bv = BMatrix::from_gradients(&mapped[qi].grad, nn, dim).dilatational_row();
                    for k in 0..ned {
                        mean_dilatation[k] += w * bv[k];
                    }
                }
                for v in mean_dilatation[..ned].iter_mut() {
                    *v /= volume;
                }
            }

            for (qi, q) in self.rule.iter().enumerate() {
                let mp = mapped[qi];
                let w = q.weight * mp.det_j;
                let mut bm = BMatrix::from_gradients(&mp.grad, nn, dim);
                if bbar {
                    bm.apply_mean_dilatation(&mean_dilatation);
                }
                let eps = bm.strain(&ue);

                let gp = e * nq + qi;
                let up: StressUpdate = self.material.update(eps, &self.state[gp], plane).map_err(
                    |err| match err {
                        FemError::ConstitutiveNotConverged {
                            iterations,
                            residual,
                            ..
                        } => FemError::ConstitutiveNotConverged {
                            element: e,
                            point: qi,
                            iterations,
                            residual,
                        },
                        other => other,
                    },
                )?;
                if up.yielding {
                    n_yield += 1;
                }
                self.trial_state[gp] = up.state;
                self.stress[gp] = up.stress;

                bm.accumulate_internal_force(&up.stress, w, &mut fe);
                if mat.is_some() {
                    bm.accumulate_stiffness(&up.tangent, w, &mut ke);
                }

                if !matches!(self.body_force, BodyForce::None) {
                    let x = map_point(et, &ec, q.xi);
                    let bvec = self.body_force.at(x);
                    let n = et.shape_functions(q.xi);
                    for a in 0..nn {
                        for c in 0..dim {
                            fb[a * dim + c] += w * n[a] * bvec[c];
                        }
                    }
                }
            }

            for (i, &g) in ed.iter().enumerate() {
                f_int[g] += fe[i];
                f_body[g] += fb[i];
            }
            if let Some(k) = mat.as_deref_mut() {
                k.scatter(&ed, &ke)?;
            }
        }

        Ok(Assembled {
            internal_force: f_int,
            body_force: f_body,
            n_yielding: n_yield,
        })
    }

    /// `L2` and `H1`-seminorm errors of `u` against the manufactured solution
    /// carried by this system's [`BodyForce`].
    ///
    /// Integrated with [`crate::quadrature::error_rule`], deliberately richer
    /// than the assembly rule: the integrand `(u_h - u)^2` is not a polynomial,
    /// and using the assembly rule would measure the quadrature error of the
    /// norm rather than the discretisation error of the solution.
    ///
    /// # Returns
    ///
    /// `(l2, h1_seminorm)` where
    /// `l2 = sqrt(integral |u_h - u|^2 dV)` \[m * m^(dim/2)\] and
    /// `h1 = sqrt(integral |grad u_h - grad u|^2 dV)` \[m^(dim/2)\].
    /// Both are absolute, not relative; the convergence study takes ratios, for
    /// which the normalisation cancels.
    ///
    /// # Errors
    ///
    /// [`FemError::BoundaryCondition`] if this system has no manufactured
    /// solution, and [`FemError::DegenerateElement`] for a bad element.
    pub fn manufactured_errors(&self, u: &[f64]) -> Result<(f64, f64)> {
        if self
            .body_force
            .manufactured_solution([0.1, 0.2, 0.0])
            .is_none()
        {
            return Err(FemError::BoundaryCondition(
                "manufactured_errors: this system has no manufactured solution".into(),
            ));
        }
        let et = self.mesh.element_type();
        let (nn, dim) = (et.n_nodes(), et.dim());
        let rule = error_rule(et);
        let mut ec = [[0.0_f64; 3]; MAX_ELEM_NODES];
        let mut ed = vec![0usize; nn * dim];
        let mut l2 = 0.0;
        let mut h1 = 0.0;

        for e in 0..self.mesh.n_elements() {
            self.mesh.element_coords(ElemId(e), &mut ec);
            self.dofs.element_dofs(&self.mesh, ElemId(e), &mut ed);
            for q in &rule {
                let mp = map_gradients(et, &ec, q.xi)?;
                let w = q.weight * mp.det_j;
                let x = map_point(et, &ec, q.xi);
                let (u_ex, g_ex) = self.body_force.manufactured_solution(x).unwrap();
                let n = et.shape_functions(q.xi);

                let mut uh = [0.0_f64; 3];
                let mut gh = [[0.0_f64; 3]; 3];
                for a in 0..nn {
                    for c in 0..dim {
                        let v = u[ed[a * dim + c]];
                        uh[c] += n[a] * v;
                        for j in 0..dim {
                            gh[c][j] += mp.grad[a][j] * v;
                        }
                    }
                }
                for c in 0..dim {
                    l2 += w * (uh[c] - u_ex[c]).powi(2);
                    for j in 0..dim {
                        h1 += w * (gh[c][j] - g_ex[c][j]).powi(2);
                    }
                }
            }
        }
        Ok((l2.sqrt(), h1.sqrt()))
    }
}

/// Interpolate a nodal field onto a physical point of one element, for
/// post-processing.
///
/// # Arguments
///
/// - `mesh` — the mesh.
/// - `e` — the element.
/// - `xi` — natural coordinate, dimensionless.
/// - `u` — the global nodal field \[m\] with `dim` components per node.
///
/// # Returns
///
/// The interpolated value \[m\], with entries beyond `dim` zero.
#[must_use]
pub fn interpolate(mesh: &Mesh, e: ElemId, xi: [f64; 3], u: &[f64]) -> [f64; 3] {
    let et: ElementType = mesh.element_type();
    let dim = et.dim();
    let n = et.shape_functions(xi);
    let mut out = [0.0; 3];
    for (a, &node) in mesh.element_nodes(e).iter().enumerate() {
        for c in 0..dim {
            out[c] += n[a] * u[node * dim + c];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{unit_cube_hex8, unit_square_quad4};

    /// A rigid-body translation must produce exactly zero strain, zero stress
    /// and zero internal force. The cheapest possible sanity check on `B`.
    #[test]
    fn rigid_translation_produces_no_force() {
        let mesh = unit_square_quad4(3).unwrap().shared();
        let mat = Material::elastic(200.0e9, 0.3).unwrap();
        let mut sys = System::new(mesh, mat, BodyForce::None);
        let n = sys.n_dofs();
        let mut u = vec![0.0; n];
        for i in (0..n).step_by(2) {
            u[i] = 1.0e-3; // uniform x translation
            u[i + 1] = -2.0e-3;
        }
        let a = sys.assemble(&u, None).unwrap();
        let maxf = a.internal_force.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        assert!(maxf < 1e-6, "rigid translation gave force {} N", maxf);
    }

    /// The elastic stiffness matrix must be symmetric and positive
    /// semi-definite, with exactly `dim (dim + 1) / 2` rigid-body modes in its
    /// null space. Checked here through the translation modes.
    #[test]
    fn stiffness_is_symmetric_and_annihilates_translations() {
        for mesh in [
            unit_square_quad4(2).unwrap().shared(),
            unit_cube_hex8(2).unwrap().shared(),
        ] {
            let dim = mesh.dim();
            let mat = Material::elastic(70.0e9, 0.33).unwrap();
            let mut sys = System::new(mesh, mat, BodyForce::None);
            let mut k = sys.new_matrix();
            let u = vec![0.0; sys.n_dofs()];
            sys.assemble(&u, Some(&mut k)).unwrap();
            let asym = k.max_asymmetry() / k.max_abs_diagonal();
            assert!(asym < 1e-12, "asymmetry {}", asym);

            for c in 0..dim {
                let mut t = vec![0.0; sys.n_dofs()];
                for i in (c..sys.n_dofs()).step_by(dim) {
                    t[i] = 1.0;
                }
                let mut y = vec![0.0; sys.n_dofs()];
                k.spmv(&t, &mut y);
                let m = y.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
                assert!(
                    m / k.max_abs_diagonal() < 1e-12,
                    "translation mode {} not in the null space: {}",
                    c,
                    m
                );
            }
        }
    }

    /// The tangent must be the exact derivative of the internal force, checked
    /// by central differences on the global residual. This is the assembly-level
    /// counterpart of the material-level tangent check, and it catches a wrong
    /// `B^T D B` ordering that the material test cannot see.
    #[test]
    fn global_tangent_matches_finite_difference_of_internal_force() {
        let mesh = unit_square_quad4(2).unwrap().shared();
        let mat = Material::j2_linear_hardening(200.0e9, 0.3, 250.0e6, 2.0e9).unwrap();
        let mut sys = System::new(mesh, mat, BodyForce::None);
        let n = sys.n_dofs();
        // A displacement large enough that several points are plastic.
        let mut u = vec![0.0; n];
        for i in 0..n {
            u[i] = 3.0e-3 * (((i * 37) % 11) as f64 / 11.0 - 0.4);
        }
        let mut k = sys.new_matrix();
        let a0 = sys.assemble(&u, Some(&mut k)).unwrap();
        assert!(a0.n_yielding > 0, "the check point must be plastic");

        let h = 1e-9;
        let scale = k.max_abs_diagonal();
        for j in [0usize, 3, 7, n - 2] {
            let mut up = u.clone();
            let mut um = u.clone();
            up[j] += h;
            um[j] -= h;
            let fp = sys.assemble(&up, None).unwrap().internal_force;
            let fm = sys.assemble(&um, None).unwrap().internal_force;
            for i in 0..n {
                let fd = (fp[i] - fm[i]) / (2.0 * h);
                assert!(
                    (fd - k.get(i, j)).abs() / scale < 1e-5,
                    "K[{}][{}] = {} but fd = {}",
                    i,
                    j,
                    k.get(i, j),
                    fd
                );
            }
        }
    }

    /// Both manufactured body forces must equal `-div sigma` of their own
    /// exact displacement field, checked by central-differencing the stress
    /// built from the analytic gradient.
    ///
    /// This is the guard that makes the convergence studies interpretable: a
    /// mis-derived body force would show up there as a lost order, which is
    /// indistinguishable from a genuine element defect. Here it fails
    /// immediately and points at the derivation instead.
    ///
    /// Measured 2026-09-11: worst relative residual 8.5e-9 (2-D) and 1.3e-8
    /// (3-D) against a `1e-6` band, which is the accuracy a central difference
    /// at `h = 1e-5` on a `pi`-scaled trigonometric field can give.
    #[test]
    fn manufactured_body_forces_are_minus_divergence_of_their_own_stress() {
        let (lambda, mu) = (115.384615e9, 76.923077e9);
        for bf in [
            BodyForce::ManufacturedSine(lambda, mu),
            BodyForce::ManufacturedSine3d(lambda, mu),
        ] {
            // sigma_ij(x) from the analytic gradient of the exact field.
            let sigma = |x: [f64; 3]| -> [[f64; 3]; 3] {
                let (_, g) = bf.manufactured_solution(x).unwrap();
                let tr = g[0][0] + g[1][1] + g[2][2];
                let mut s = [[0.0_f64; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        s[i][j] = mu * (g[i][j] + g[j][i]);
                    }
                    s[i][i] += lambda * tr;
                }
                s
            };
            let x0 = [0.37, 0.61, 0.23];
            let h = 1.0e-5;
            let mut div = [0.0_f64; 3];
            for j in 0..3 {
                let (mut xp, mut xm) = (x0, x0);
                xp[j] += h;
                xm[j] -= h;
                let (sp, sm) = (sigma(xp), sigma(xm));
                for (i, d) in div.iter_mut().enumerate() {
                    *d += (sp[i][j] - sm[i][j]) / (2.0 * h);
                }
            }
            let f = bf.at(x0);
            let scale = f.iter().fold(0.0_f64, |m, v| m.max(v.abs())).max(1.0);
            for i in 0..3 {
                let rel = (f[i] + div[i]).abs() / scale;
                assert!(
                    rel < 1e-6,
                    "{bf:?} component {i}: f = {}, -div sigma = {}, relative {rel:e}",
                    f[i],
                    -div[i]
                );
            }
        }
    }

    /// B-bar must be a **no-op on a constant-strain element**: Tri3 gradients
    /// do not vary within the element, so the element mean of the dilatational
    /// row is the row itself. This is the property that makes B-bar useless for
    /// Tri3/Tet4, and it is asserted rather than merely claimed in a doc
    /// comment.
    ///
    /// Measured 2026-09-11: largest entry-wise difference between the
    /// full-integration and B-bar stiffness matrices is 3.05e-5 N/m on a Tri3
    /// unit square, against a matrix scale of 6.9e11 N/m — a relative
    /// difference of 4.41e-17, i.e. a fraction of one unit in the last place. It is not
    /// *bit*-identical because the mean dilatation is formed as a weighted sum
    /// divided by the element volume rather than read off directly, and that
    /// round-trip is not exact in binary floating point. The pass band is 1e-14
    /// relative, two orders above the observed value and eleven below anything
    /// a real formulation change could produce.
    #[test]
    fn bbar_is_exactly_a_no_op_on_constant_strain_triangles() {
        use crate::mesh::unit_square_tri3;
        let mesh = unit_square_tri3(3).unwrap().shared();
        let mat = Material::elastic(200.0e9, 0.3).unwrap();
        let mut full = System::new(Arc::clone(&mesh), mat, BodyForce::None);
        let mut bbar = System::with_options(
            Arc::clone(&mesh),
            mat,
            BodyForce::None,
            SystemOptions {
                formulation: Formulation::BBar,
                ..SystemOptions::default()
            },
        )
        .unwrap();
        let u = vec![0.0; full.n_dofs()];
        let (mut kf, mut kb) = (full.new_matrix(), bbar.new_matrix());
        full.assemble(&u, Some(&mut kf)).unwrap();
        bbar.assemble(&u, Some(&mut kb)).unwrap();
        let worst = kf
            .values()
            .iter()
            .zip(kb.values())
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        let relative = worst / kf.max_abs_diagonal();
        println!(
            "B-bar vs full integration on Tri3: {worst:e} N/m absolute, {relative:e} relative"
        );
        assert!(
            relative < 1e-14,
            "B-bar changed a constant-strain element's stiffness by {worst} N/m \
             ({relative:e} relative), which is far beyond round-off"
        );
    }

    /// On a Quad4, by contrast, B-bar must genuinely change the element — the
    /// control that shows the previous test is measuring a property of the
    /// element and not a formulation that never does anything.
    ///
    /// Measured 2026-09-11 at `nu = 0.3`: the largest entry-wise difference is
    /// **12.0 %** of the largest diagonal entry of the full-integration matrix
    /// — four orders of magnitude clear of the `1e-14` round-off band the Tri3
    /// case sits in. The pass band is 5 %.
    #[test]
    fn bbar_genuinely_changes_a_bilinear_quadrilateral() {
        let mesh = unit_square_quad4(3).unwrap().shared();
        let mat = Material::elastic(200.0e9, 0.3).unwrap();
        let mut full = System::new(Arc::clone(&mesh), mat, BodyForce::None);
        let mut bbar = System::with_options(
            Arc::clone(&mesh),
            mat,
            BodyForce::None,
            SystemOptions {
                formulation: Formulation::BBar,
                ..SystemOptions::default()
            },
        )
        .unwrap();
        let u = vec![0.0; full.n_dofs()];
        let (mut kf, mut kb) = (full.new_matrix(), bbar.new_matrix());
        full.assemble(&u, Some(&mut kf)).unwrap();
        bbar.assemble(&u, Some(&mut kb)).unwrap();
        let worst = kf
            .values()
            .iter()
            .zip(kb.values())
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        let scale = kf.max_abs_diagonal();
        println!("B-bar vs full integration on Quad4: {:.3} of the largest diagonal", worst / scale);
        assert!(worst / scale > 0.05, "B-bar barely changed Quad4: {}", worst / scale);
        // And it must stay symmetric.
        assert!(kb.max_asymmetry() / kb.max_abs_diagonal() < 1e-12);
    }

    /// The B-bar tangent must remain the **exact** derivative of the B-bar
    /// internal force, including through the J2 return map. If it were not, the
    /// quadratic Newton convergence the suite measures would be destroyed, so
    /// this is checked at the assembly level by central differences exactly as
    /// the full-integration case is.
    ///
    /// Measured 2026-09-11: worst relative entry error **1.735e-10** against a
    /// `1e-5` band, on a Quad4 mesh with points in plastic flow. That is the
    /// central difference's own accuracy, not the tangent's.
    #[test]
    fn bbar_global_tangent_matches_finite_difference_of_internal_force() {
        let mesh = unit_square_quad4(2).unwrap().shared();
        let mat = Material::j2_linear_hardening(200.0e9, 0.3, 250.0e6, 2.0e9).unwrap();
        let mut sys = System::with_options(
            mesh,
            mat,
            BodyForce::None,
            SystemOptions {
                formulation: Formulation::BBar,
                ..SystemOptions::default()
            },
        )
        .unwrap();
        let n = sys.n_dofs();
        let mut u = vec![0.0; n];
        for i in 0..n {
            u[i] = 3.0e-3 * (((i * 37) % 11) as f64 / 11.0 - 0.4);
        }
        let mut k = sys.new_matrix();
        let a0 = sys.assemble(&u, Some(&mut k)).unwrap();
        assert!(a0.n_yielding > 0, "the check point must be plastic");

        let h = 1e-9;
        let scale = k.max_abs_diagonal();
        let mut worst = 0.0_f64;
        for j in [0usize, 3, 7, n - 2] {
            let (mut up, mut um) = (u.clone(), u.clone());
            up[j] += h;
            um[j] -= h;
            let fp = sys.assemble(&up, None).unwrap().internal_force;
            let fm = sys.assemble(&um, None).unwrap().internal_force;
            for i in 0..n {
                let fd = (fp[i] - fm[i]) / (2.0 * h);
                worst = worst.max((fd - k.get(i, j)).abs() / scale);
            }
        }
        println!("B-bar global tangent vs central difference: {worst:.3e} relative");
        assert!(worst < 1e-5, "B-bar tangent is not consistent: {worst:e}");
    }

    /// The two rejected option combinations must be rejected, with an error
    /// that says why — not silently accepted and not silently ignored.
    ///
    /// Both refusals are deliberate, not gaps: plane stress has no meaning on a
    /// three-dimensional mesh, and B-bar has nothing to relax under plane
    /// stress because the out-of-plane strain is already free. An unverified
    /// combination that runs is worse than one that refuses.
    #[test]
    fn invalid_option_combinations_are_rejected() {
        let mat = Material::elastic(200.0e9, 0.3).unwrap();
        let cube = unit_cube_hex8(2).unwrap().shared();
        let square = unit_square_quad4(2).unwrap().shared();

        let e = System::with_options(
            Arc::clone(&cube),
            mat,
            BodyForce::None,
            SystemOptions {
                plane_condition: PlaneCondition::PlaneStress,
                ..SystemOptions::default()
            },
        )
        .expect_err("plane stress on a 3-D mesh must be rejected");
        assert!(format!("{e}").contains("three-dimensional"), "{e}");

        let e = System::with_options(
            Arc::clone(&square),
            mat,
            BodyForce::None,
            SystemOptions {
                formulation: Formulation::BBar,
                plane_condition: PlaneCondition::PlaneStress,
            },
        )
        .expect_err("B-bar with plane stress must be rejected");
        assert!(format!("{e}").contains("volumetric constraint"), "{e}");

        // And the four valid combinations must all build.
        for (mesh, options) in [
            (Arc::clone(&square), SystemOptions::default()),
            (
                Arc::clone(&square),
                SystemOptions {
                    formulation: Formulation::BBar,
                    plane_condition: PlaneCondition::PlaneStrain,
                },
            ),
            (
                Arc::clone(&square),
                SystemOptions {
                    formulation: Formulation::FullIntegration,
                    plane_condition: PlaneCondition::PlaneStress,
                },
            ),
            (
                Arc::clone(&cube),
                SystemOptions {
                    formulation: Formulation::BBar,
                    plane_condition: PlaneCondition::PlaneStrain,
                },
            ),
        ] {
            let sys = System::with_options(mesh, mat, BodyForce::None, options)
                .unwrap_or_else(|e| panic!("{options:?} should be valid: {e}"));
            assert_eq!(sys.options(), options);
            assert_eq!(sys.formulation(), options.formulation);
            assert_eq!(sys.plane_condition(), options.plane_condition);
        }

        // `System::new` must be exactly the default options.
        let sys = System::new(square, mat, BodyForce::None);
        assert_eq!(sys.options(), SystemOptions::default());
        assert_eq!(sys.formulation(), Formulation::FullIntegration);
        assert_eq!(sys.plane_condition(), PlaneCondition::PlaneStrain);
    }

    /// The body-force integral of a uniform load must equal load times volume,
    /// component by component — a check on the `N^T b` quadrature.
    #[test]
    fn uniform_body_force_integrates_to_load_times_volume() {
        let mesh = unit_cube_hex8(2).unwrap().shared();
        let mat = Material::elastic(1.0e9, 0.25).unwrap();
        let b = [3.0, -5.0, 7.0];
        let mut sys = System::new(mesh, mat, BodyForce::Uniform(b));
        let u = vec![0.0; sys.n_dofs()];
        let a = sys.assemble(&u, None).unwrap();
        for c in 0..3 {
            let total: f64 = a.body_force.iter().skip(c).step_by(3).sum();
            assert!((total - b[c]).abs() < 1e-9, "component {}: {}", c, total);
        }
    }
}
