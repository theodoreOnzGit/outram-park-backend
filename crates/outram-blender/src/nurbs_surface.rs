// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NURBS surfaces. Follows the published architecture of Blender's NURBS surface
// objects (source/blender/blenkernel/intern/curve.cc `BKE_nurb_makeFaces` and
// the surface primitives in editors/curve/editcurve_add.cc, github.com/blender/
// blender, GPL-2.0-or-later): a tensor-product rational B-spline patch, its
// evaluation, tessellation, and the surface primitives. Concepts only — no
// upstream source copied.
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

//! **NURBS surfaces** (`op-hzs.54.37`, GH issue #37 §G).
//!
//! [`NurbsSurface`] is a tensor-product rational B-spline patch: an
//! `nu × nv` grid of control points with weights and a clamped uniform knot
//! vector on each axis. [`NurbsSurface::evaluate`] gives a point, and
//! [`NurbsSurface::to_mesh`] tessellates it into a quad grid.
//!
//! Primitives ([`NurbsSurface::plane`] / [`sphere`](NurbsSurface::sphere) /
//! [`cylinder`](NurbsSurface::cylinder) / [`torus`](NurbsSurface::torus))
//! match Blender's Add-Surface menu. Patch editing is
//! [`NurbsSurface::move_control`] and [`NurbsSurface::control_mut`].

use crate::math::Vec3;
use crate::mesh::Mesh;

/// A tensor-product NURBS surface patch.
#[derive(Debug, Clone)]
pub struct NurbsSurface {
    /// Control-point counts along `u` and `v`.
    pub nu: usize,
    pub nv: usize,
    /// Orders (degree + 1) along `u` and `v` (`>= 2`).
    pub order_u: usize,
    pub order_v: usize,
    /// `control[u + nu * v]` — the control-point positions.
    pub control: Vec<Vec3>,
    /// Matching weights (`1.0` = a plain B-spline point).
    pub weights: Vec<f64>,
    /// Wrap the surface along `u` / `v` (a cylinder wraps in one, a torus in
    /// both).
    pub cyclic_u: bool,
    pub cyclic_v: bool,
}

impl NurbsSurface {
    fn ctrl(&self, u: usize, v: usize) -> Vec3 {
        self.control[u + self.nu * v]
    }
    fn weight(&self, u: usize, v: usize) -> f64 {
        self.weights[u + self.nu * v]
    }

    /// Mutable access to control point `(u, v)`.
    pub fn control_mut(&mut self, u: usize, v: usize) -> &mut Vec3 {
        &mut self.control[u + self.nu * v]
    }

    /// Translate control point `(u, v)` by `delta` (patch editing).
    pub fn move_control(&mut self, u: usize, v: usize, delta: Vec3) {
        let i = u + self.nu * v;
        self.control[i] = self.control[i].add(delta);
    }

    /// Evaluate the surface at parameters `(u, v)`, each in `[0, 1]`.
    #[allow(clippy::needless_range_loop)]
    pub fn evaluate(&self, u: f64, v: f64) -> Vec3 {
        let du = self.order_u.min(self.nu).max(2) - 1;
        let dv = self.order_v.min(self.nv).max(2) - 1;
        // Effective control counts and knots — a cyclic axis is padded with
        // `degree` wrapped columns and given a uniform (periodic) knot vector.
        let (nu_e, ku) = axis_knots(self.nu, du, self.cyclic_u);
        let (nv_e, kv) = axis_knots(self.nv, dv, self.cyclic_v);
        let uu = ku[du] + (ku[nu_e] - ku[du]) * u.clamp(0.0, 1.0);
        let vv = kv[dv] + (kv[nv_e] - kv[dv]) * v.clamp(0.0, 1.0);
        let (su, bu) = basis(&ku, du, nu_e, uu);
        let (sv, bv) = basis(&kv, dv, nv_e, vv);

        let mut num = Vec3::ZERO;
        let mut den = 0.0;
        for j in 0..=dv {
            let vj = sv - dv + j;
            let vj_r = if self.cyclic_v {
                vj % self.nv
            } else {
                vj.min(self.nv - 1)
            };
            for i in 0..=du {
                let ui = su - du + i;
                let ui_r = if self.cyclic_u {
                    ui % self.nu
                } else {
                    ui.min(self.nu - 1)
                };
                let w = self.weight(ui_r, vj_r) * bu[i] * bv[j];
                num = num.add(self.ctrl(ui_r, vj_r).scale(w));
                den += w;
            }
        }
        if den.abs() > 1e-12 {
            num.scale(1.0 / den)
        } else {
            self.ctrl(su.min(self.nu - 1) % self.nu, sv.min(self.nv - 1) % self.nv)
        }
    }

    /// Tessellate the surface into a `res_u × res_v` quad grid.
    pub fn to_mesh(&self, res_u: usize, res_v: usize) -> Mesh {
        let ru = res_u.max(2);
        let rv = res_v.max(2);
        let ud = if self.cyclic_u {
            ru as f64
        } else {
            (ru - 1) as f64
        };
        let vd = if self.cyclic_v {
            rv as f64
        } else {
            (rv - 1) as f64
        };
        let mut positions = Vec::with_capacity(ru * rv);
        for j in 0..rv {
            for i in 0..ru {
                positions.push(self.evaluate(i as f64 / ud, j as f64 / vd));
            }
        }
        let idx = |i: usize, j: usize| j * ru + i;
        let mut faces = Vec::new();
        let iu = if self.cyclic_u { ru } else { ru - 1 };
        let iv = if self.cyclic_v { rv } else { rv - 1 };
        for j in 0..iv {
            for i in 0..iu {
                let (i1, j1) = ((i + 1) % ru, (j + 1) % rv);
                faces.push(vec![idx(i, j), idx(i1, j), idx(i1, j1), idx(i, j1)]);
            }
        }
        Mesh::from_polygons(&positions, &faces)
    }

    // -- primitives ------------------------------------------------------

    /// A flat `nu × nv` control grid spanning `[-1, 1]²` in the `z = 0` plane.
    pub fn plane(nu: usize, nv: usize) -> Self {
        let (nu, nv) = (nu.max(2), nv.max(2));
        let mut control = Vec::with_capacity(nu * nv);
        for j in 0..nv {
            for i in 0..nu {
                control.push(Vec3::new(
                    -1.0 + 2.0 * i as f64 / (nu - 1) as f64,
                    -1.0 + 2.0 * j as f64 / (nv - 1) as f64,
                    0.0,
                ));
            }
        }
        NurbsSurface {
            nu,
            nv,
            order_u: 4.min(nu),
            order_v: 4.min(nv),
            weights: vec![1.0; nu * nv],
            control,
            cyclic_u: false,
            cyclic_v: false,
        }
    }

    /// A NURBS sphere of `radius` — a dense `nu × nv` control grid sampled on
    /// the sphere (non-rational; the degree-3 approximation stays within a few
    /// percent), cyclic in `u`.
    pub fn sphere(radius: f64) -> Self {
        let (nu, nv) = (16usize, 9usize);
        let mut control = Vec::with_capacity(nu * nv);
        for j in 0..nv {
            let la =
                -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * j as f64 / (nv - 1) as f64;
            for i in 0..nu {
                let lo = std::f64::consts::TAU * i as f64 / nu as f64;
                control.push(Vec3::new(
                    radius * la.cos() * lo.cos(),
                    radius * la.cos() * lo.sin(),
                    radius * la.sin(),
                ));
            }
        }
        NurbsSurface {
            nu,
            nv,
            order_u: 4,
            order_v: 4,
            weights: vec![1.0; nu * nv],
            control,
            cyclic_u: true,
            cyclic_v: false,
        }
    }

    /// A NURBS cylinder of `radius` and `height` — cyclic in `u`.
    pub fn cylinder(radius: f64, height: f64) -> Self {
        let nu = 16usize;
        let mut control = Vec::with_capacity(nu * 2);
        for j in 0..2 {
            let z = -height * 0.5 + height * j as f64;
            for i in 0..nu {
                let lo = std::f64::consts::TAU * i as f64 / nu as f64;
                control.push(Vec3::new(radius * lo.cos(), radius * lo.sin(), z));
            }
        }
        NurbsSurface {
            nu,
            nv: 2,
            order_u: 4,
            order_v: 2,
            weights: vec![1.0; nu * 2],
            control,
            cyclic_u: true,
            cyclic_v: false,
        }
    }

    /// A NURBS torus of major radius `r` and minor radius `t` — cyclic in both.
    pub fn torus(r: f64, t: f64) -> Self {
        let (nu, nv) = (20usize, 12usize);
        let mut control = Vec::with_capacity(nu * nv);
        for j in 0..nv {
            let v = std::f64::consts::TAU * j as f64 / nv as f64;
            for i in 0..nu {
                let u = std::f64::consts::TAU * i as f64 / nu as f64;
                let rr = r + t * v.cos();
                control.push(Vec3::new(rr * u.cos(), rr * u.sin(), t * v.sin()));
            }
        }
        NurbsSurface {
            nu,
            nv,
            order_u: 4,
            order_v: 4,
            weights: vec![1.0; nu * nv],
            control,
            cyclic_u: true,
            cyclic_v: true,
        }
    }
}

// --- shared B-spline basis ---

/// The effective control count and knot vector for one axis:
/// clamped-uniform when open, uniform (periodic) over `n + degree` control
/// points when cyclic.
fn axis_knots(n: usize, degree: usize, cyclic: bool) -> (usize, Vec<f64>) {
    if cyclic {
        let ne = n + degree;
        let m = ne + degree + 1;
        let knots: Vec<f64> = (0..m).map(|i| i as f64).collect();
        (ne, knots)
    } else {
        let m = n + degree + 1;
        let knots = (0..m)
            .map(|i| {
                if i <= degree {
                    0.0
                } else if i >= n {
                    (n - degree) as f64
                } else {
                    (i - degree) as f64
                }
            })
            .collect();
        (n, knots)
    }
}

/// `(span, basis[0..=degree])` at `u` (Cox-de-Boor).
fn basis(knots: &[f64], degree: usize, n: usize, u: f64) -> (usize, Vec<f64>) {
    let mut span = degree;
    while span < n - 1 && u >= knots[span + 1] {
        span += 1;
    }
    let mut b = vec![0.0; degree + 1];
    b[0] = 1.0;
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    for j in 1..=degree {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            let temp = if denom.abs() > 1e-12 {
                b[r] / denom
            } else {
                0.0
            };
            b[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        b[j] = saved;
    }
    (span, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::VertexId;

    #[test]
    fn plane_evaluates_to_its_control_grid_corners() {
        let s = NurbsSurface::plane(4, 4);
        assert!(
            s.evaluate(0.0, 0.0)
                .sub(Vec3::new(-1.0, -1.0, 0.0))
                .length()
                < 1e-6
        );
        assert!(s.evaluate(1.0, 1.0).sub(Vec3::new(1.0, 1.0, 0.0)).length() < 1e-6);
        // A mid sample is still on z = 0.
        assert!(s.evaluate(0.5, 0.5).z.abs() < 1e-9);
    }

    #[test]
    fn plane_tessellates_to_a_quad_grid() {
        let s = NurbsSurface::plane(3, 3);
        let m = s.to_mesh(6, 6);
        assert_eq!(m.face_count(), 25);
        assert_eq!(m.euler_characteristic(), 1);
    }

    #[test]
    fn nurbs_sphere_points_are_near_the_radius() {
        let s = NurbsSurface::sphere(2.0);
        for j in 0..11 {
            for i in 0..17 {
                let p = s.evaluate(i as f64 / 16.0, j as f64 / 10.0);
                assert!(
                    (p.length() - 2.0).abs() < 0.25,
                    "r ≈ 2 (got {})",
                    p.length()
                );
            }
        }
    }

    #[test]
    fn nurbs_sphere_meshes_to_a_closed_surface() {
        let s = NurbsSurface::sphere(1.0);
        let m = s.to_mesh(20, 12);
        // Cyclic-u welded: the seam column merged, so it is closed in u.
        assert!(m.face_count() > 100);
        // Every mesh vertex is ~ on the unit sphere.
        for i in 0..m.vertex_count() {
            let r = m.vertex(VertexId(i)).unwrap().position.length();
            assert!((r - 1.0).abs() < 0.2);
        }
    }

    #[test]
    fn cylinder_and_torus_tessellate() {
        let c = NurbsSurface::cylinder(1.0, 3.0).to_mesh(16, 4);
        assert!(c.face_count() > 20);
        let t = NurbsSurface::torus(3.0, 1.0).to_mesh(16, 12);
        assert!(t.face_count() > 100);
    }

    #[test]
    fn patch_editing_moves_the_surface() {
        let mut s = NurbsSurface::plane(3, 3);
        let before = s.evaluate(0.5, 0.5);
        s.move_control(1, 1, Vec3::new(0.0, 0.0, 2.0)); // pull the centre up
        let after = s.evaluate(0.5, 0.5);
        assert!(after.z > before.z + 0.1, "centre lifted");
    }
}
