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

//! Gauss quadrature rules on the reference elements, **with their exactness
//! order stated for every rule**.
//!
//! # What belongs in this module
//!
//! Point/weight tables on the reference domains of [`crate::element`], and the
//! default rule each element type uses for stiffness assembly.
//!
//! # What does NOT belong here
//!
//! The Jacobian, which converts a reference-domain weight into a physical
//! measure — that is [`crate::element::map_gradients`]. Weights here are
//! reference-domain weights and sum to the reference measure, not to one.
//!
//! # Exactness order of every rule implemented
//!
//! "Exact to degree `d`" means the rule integrates every polynomial of total
//! degree `<= d` over the reference domain with no error beyond rounding. Each
//! claim below is checked by a test in this module against the closed-form
//! monomial integral, not merely asserted.
//!
//! ## Line, `[-1, 1]`, weights sum to 2 (used for facet tractions)
//!
//! | Points | Exact to degree | Notes |
//! |---|---|---|
//! | 1 | 1 | midpoint |
//! | 2 | 3 | |
//! | 3 | 5 | |
//!
//! ## Quadrilateral, `[-1, 1]^2`, weights sum to 4
//!
//! Tensor products of the line rules.
//!
//! | Rule | Points | Exact to degree (per direction) |
//! |---|---|---|
//! | 1x1 | 1 | 1 |
//! | 2x2 | 4 | 3 |
//! | 3x3 | 9 | 5 |
//!
//! ## Hexahedron, `[-1, 1]^3`, weights sum to 8
//!
//! | Rule | Points | Exact to degree (per direction) |
//! |---|---|---|
//! | 1x1x1 | 1 | 1 |
//! | 2x2x2 | 8 | 3 |
//! | 3x3x3 | 27 | 5 |
//!
//! ## Triangle, unit reference triangle, weights sum to 1/2
//!
//! | Points | Exact to degree | Source |
//! |---|---|---|
//! | 1 | 1 | centroid |
//! | 3 | 2 | midside / interior symmetric rule |
//! | 4 | 3 | centroid plus three, one negative weight |
//! | 6 | 4 | two symmetric orbits |
//!
//! ## Tetrahedron, unit reference tetrahedron, weights sum to 1/6
//!
//! | Points | Exact to degree | Source |
//! |---|---|---|
//! | 1 | 1 | centroid |
//! | 4 | 2 | symmetric orbit, `a = (5 - sqrt(5))/20` |
//! | 5 | 3 | centroid plus four, one negative weight |
//!
//! # Defaults, and why
//!
//! [`default_rule`] picks the cheapest rule that integrates the **stiffness**
//! integrand `B^T D B det J` exactly on an undistorted element. For a
//! distorted isoparametric element `det J` is not constant and no finite rule
//! is exact; the defaults are the standard engineering choice and are the rules
//! the verification suite was run with.
//!
//! **Reduced integration is deliberately not offered.** It would cure the
//! shear locking the cantilever study measures, at the price of hourglass modes
//! that need stabilisation — a real feature, but one that must be verified in
//! its own right rather than smuggled in as a default.
//!
//! # Units
//!
//! Natural coordinates and weights are dimensionless.

use crate::element::{ElementType, FacetType};

/// One quadrature point: a natural coordinate and its reference-domain weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadraturePoint {
    /// Natural coordinates on the reference domain, dimensionless. Entries
    /// beyond the element's dimension are zero.
    pub xi: [f64; 3],
    /// Reference-domain weight, dimensionless. The physical contribution is
    /// `weight * det_j`.
    pub weight: f64,
}

/// Gauss-Legendre points on `[-1, 1]` for `n` in 1..=3, exact to degree
/// `2n - 1`.
///
/// Returns `(abscissa, weight)` pairs whose weights sum to 2. `n` outside
/// 1..=3 falls back to `n = 3`, because every caller in this crate is
/// internal and picks `n` from a fixed table.
fn gauss_legendre_1d(n: usize) -> Vec<(f64, f64)> {
    match n {
        1 => vec![(0.0, 2.0)],
        2 => {
            let a = 1.0 / 3.0_f64.sqrt();
            vec![(-a, 1.0), (a, 1.0)]
        }
        _ => {
            let a = (3.0_f64 / 5.0).sqrt();
            vec![(-a, 5.0 / 9.0), (0.0, 8.0 / 9.0), (a, 5.0 / 9.0)]
        }
    }
}

/// Tensor-product Gauss rule on `[-1, 1]^2` with `n` points per direction.
///
/// Exact to degree `2n - 1` in each coordinate separately. Weights sum to 4.
#[must_use]
pub fn quad_rule(n: usize) -> Vec<QuadraturePoint> {
    let g = gauss_legendre_1d(n);
    let mut pts = Vec::with_capacity(g.len() * g.len());
    for (s, ws) in &g {
        for (r, wr) in &g {
            pts.push(QuadraturePoint {
                xi: [*r, *s, 0.0],
                weight: wr * ws,
            });
        }
    }
    pts
}

/// Tensor-product Gauss rule on `[-1, 1]^3` with `n` points per direction.
///
/// Exact to degree `2n - 1` in each coordinate separately. Weights sum to 8.
#[must_use]
pub fn hex_rule(n: usize) -> Vec<QuadraturePoint> {
    let g = gauss_legendre_1d(n);
    let mut pts = Vec::with_capacity(g.len().pow(3));
    for (t, wt) in &g {
        for (s, ws) in &g {
            for (r, wr) in &g {
                pts.push(QuadraturePoint {
                    xi: [*r, *s, *t],
                    weight: wr * ws * wt,
                });
            }
        }
    }
    pts
}

/// Symmetric quadrature on the unit reference triangle, weights summing to 1/2.
///
/// `n_points` must be 1, 3, 4 or 6, giving exactness to degree 1, 2, 3 and 4
/// respectively (see the module table). Any other value falls back to the
/// 6-point rule.
///
/// The 4-point rule carries one **negative** weight (`-27/96` of the area).
/// That is correct and standard, but it means the rule is not positive-definite
/// and should not be used to integrate a quantity whose sign is being relied
/// on; the crate uses it nowhere by default.
#[must_use]
pub fn triangle_rule(n_points: usize) -> Vec<QuadraturePoint> {
    let p = |a: f64, b: f64, w: f64| QuadraturePoint {
        xi: [a, b, 0.0],
        weight: w,
    };
    match n_points {
        1 => vec![p(1.0 / 3.0, 1.0 / 3.0, 0.5)],
        3 => {
            let w = 1.0 / 6.0;
            vec![
                p(1.0 / 6.0, 1.0 / 6.0, w),
                p(2.0 / 3.0, 1.0 / 6.0, w),
                p(1.0 / 6.0, 2.0 / 3.0, w),
            ]
        }
        4 => vec![
            p(1.0 / 3.0, 1.0 / 3.0, -27.0 / 96.0),
            p(0.6, 0.2, 25.0 / 96.0),
            p(0.2, 0.6, 25.0 / 96.0),
            p(0.2, 0.2, 25.0 / 96.0),
        ],
        _ => {
            // Two symmetric orbits, exact to degree 4 (Dunavant / Strang-Fix).
            let a1 = 0.445_948_490_915_965;
            let w1 = 0.223_381_589_678_011 * 0.5;
            let a2 = 0.091_576_213_509_771;
            let w2 = 0.109_951_743_655_322 * 0.5;
            vec![
                p(a1, a1, w1),
                p(1.0 - 2.0 * a1, a1, w1),
                p(a1, 1.0 - 2.0 * a1, w1),
                p(a2, a2, w2),
                p(1.0 - 2.0 * a2, a2, w2),
                p(a2, 1.0 - 2.0 * a2, w2),
            ]
        }
    }
}

/// Symmetric quadrature on the unit reference tetrahedron, weights summing to
/// 1/6.
///
/// `n_points` must be 1, 4 or 5, giving exactness to degree 1, 2 and 3. Any
/// other value falls back to the 5-point rule, which carries one negative
/// weight (see [`triangle_rule`] for the same caveat).
#[must_use]
pub fn tetrahedron_rule(n_points: usize) -> Vec<QuadraturePoint> {
    let p = |a: f64, b: f64, c: f64, w: f64| QuadraturePoint {
        xi: [a, b, c],
        weight: w,
    };
    match n_points {
        1 => vec![p(0.25, 0.25, 0.25, 1.0 / 6.0)],
        4 => {
            let a = (5.0 - 5.0_f64.sqrt()) / 20.0;
            let b = (5.0 + 3.0 * 5.0_f64.sqrt()) / 20.0;
            let w = 1.0 / 24.0;
            vec![
                p(a, a, a, w),
                p(b, a, a, w),
                p(a, b, a, w),
                p(a, a, b, w),
            ]
        }
        _ => {
            let w0 = -0.8 / 6.0;
            let w1 = 0.45 / 6.0;
            vec![
                p(0.25, 0.25, 0.25, w0),
                p(1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0, w1),
                p(0.5, 1.0 / 6.0, 1.0 / 6.0, w1),
                p(1.0 / 6.0, 0.5, 1.0 / 6.0, w1),
                p(1.0 / 6.0, 1.0 / 6.0, 0.5, w1),
            ]
        }
    }
}

/// Gauss rule on a facet, for integrating a surface traction.
///
/// | Facet | Rule | Exact to degree |
/// |---|---|---|
/// | [`FacetType::Line2`] | 2-point Gauss | 3 |
/// | [`FacetType::Line3`] | 3-point Gauss | 5 |
/// | [`FacetType::Tri3`] | 3-point triangle | 2 |
/// | [`FacetType::Quad4`] | 2x2 Gauss | 3 per direction |
///
/// In each case the rule integrates `N_a t det J_s` exactly for a constant
/// traction on an undistorted facet, with headroom for a linearly varying one.
#[must_use]
pub fn facet_rule(facet: FacetType) -> Vec<QuadraturePoint> {
    match facet {
        FacetType::Line2 => gauss_legendre_1d(2)
            .into_iter()
            .map(|(r, w)| QuadraturePoint {
                xi: [r, 0.0, 0.0],
                weight: w,
            })
            .collect(),
        FacetType::Line3 => gauss_legendre_1d(3)
            .into_iter()
            .map(|(r, w)| QuadraturePoint {
                xi: [r, 0.0, 0.0],
                weight: w,
            })
            .collect(),
        FacetType::Tri3 => triangle_rule(3),
        FacetType::Quad4 => quad_rule(2),
    }
}

/// The default stiffness-integration rule for each element type.
///
/// | Element | Rule | Points | Why |
/// |---|---|---|---|
/// | [`ElementType::Tri3`] | 1-point | 1 | the integrand `B^T D B det J` is constant |
/// | [`ElementType::Tri6`] | 6-point | 6 | degree 2 integrand on a straight-sided element; degree 4 exactness gives headroom for the body-force term and mild distortion |
/// | [`ElementType::Quad4`] | 2x2 | 4 | full integration; degree 3 per direction |
/// | [`ElementType::Tet4`] | 1-point | 1 | constant integrand |
/// | [`ElementType::Hex8`] | 2x2x2 | 8 | full integration; degree 3 per direction |
///
/// These are **full** integration rules throughout — see the module note on why
/// reduced integration is not offered.
#[must_use]
pub fn default_rule(element: ElementType) -> Vec<QuadraturePoint> {
    match element {
        ElementType::Tri3 => triangle_rule(1),
        ElementType::Tri6 => triangle_rule(6),
        ElementType::Quad4 => quad_rule(2),
        ElementType::Tet4 => tetrahedron_rule(1),
        ElementType::Hex8 => hex_rule(2),
    }
}

/// A high-order rule for **error integration**, not for assembly.
///
/// The manufactured-solution study integrates `(u_h - u_exact)^2`, whose
/// integrand is not a polynomial. Using the assembly rule there would measure
/// the quadrature error of the norm rather than the discretisation error of the
/// solution. This returns the highest rule implemented for each element:
/// 6-point triangle, 5-point tetrahedron, 3x3 quadrilateral, 3x3x3 hexahedron.
#[must_use]
pub fn error_rule(element: ElementType) -> Vec<QuadraturePoint> {
    match element {
        ElementType::Tri3 | ElementType::Tri6 => triangle_rule(6),
        ElementType::Quad4 => quad_rule(3),
        ElementType::Tet4 => tetrahedron_rule(5),
        ElementType::Hex8 => hex_rule(3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact integral of `x^i y^j` over the unit reference triangle:
    /// `i! j! / (i + j + 2)!`.
    fn triangle_monomial(i: u32, j: u32) -> f64 {
        fn fact(n: u32) -> f64 {
            (1..=n).map(f64::from).product::<f64>().max(1.0)
        }
        fact(i) * fact(j) / fact(i + j + 2)
    }

    /// Exact integral of `x^i y^j z^k` over the unit reference tetrahedron:
    /// `i! j! k! / (i + j + k + 3)!`.
    fn tet_monomial(i: u32, j: u32, k: u32) -> f64 {
        fn fact(n: u32) -> f64 {
            (1..=n).map(f64::from).product::<f64>().max(1.0)
        }
        fact(i) * fact(j) * fact(k) / fact(i + j + k + 3)
    }

    /// Exact integral of `x^i` over `[-1, 1]`.
    fn line_monomial(i: u32) -> f64 {
        if i % 2 == 1 {
            0.0
        } else {
            2.0 / f64::from(i + 1)
        }
    }

    /// Every weight set must sum to its reference measure — the degree-0 case,
    /// and the first thing a transcription error breaks.
    #[test]
    fn weights_sum_to_reference_measure() {
        for n in [1, 3, 4, 6] {
            let s: f64 = triangle_rule(n).iter().map(|p| p.weight).sum();
            assert!((s - 0.5).abs() < 1e-14, "triangle {} sum {}", n, s);
        }
        for n in [1, 4, 5] {
            let s: f64 = tetrahedron_rule(n).iter().map(|p| p.weight).sum();
            assert!((s - 1.0 / 6.0).abs() < 1e-14, "tet {} sum {}", n, s);
        }
        for n in 1..=3 {
            let s: f64 = quad_rule(n).iter().map(|p| p.weight).sum();
            assert!((s - 4.0).abs() < 1e-14);
            let s3: f64 = hex_rule(n).iter().map(|p| p.weight).sum();
            assert!((s3 - 8.0).abs() < 1e-13);
        }
    }

    /// The exactness order claimed in the module table for the **triangle**
    /// rules, verified monomial by monomial.
    #[test]
    fn triangle_exactness_orders_hold() {
        for (n, deg) in [(1usize, 1u32), (3, 2), (4, 3), (6, 4)] {
            let rule = triangle_rule(n);
            for i in 0..=deg {
                for j in 0..=(deg - i) {
                    let num: f64 = rule
                        .iter()
                        .map(|p| p.weight * p.xi[0].powi(i as i32) * p.xi[1].powi(j as i32))
                        .sum();
                    let exact = triangle_monomial(i, j);
                    assert!(
                        (num - exact).abs() < 1e-13,
                        "tri {}-pt, x^{} y^{}: {} vs {}",
                        n,
                        i,
                        j,
                        num,
                        exact
                    );
                }
            }
        }
    }

    /// One degree beyond the claim must actually fail, or the claim is
    /// understated and the table is wrong in the other direction.
    #[test]
    fn triangle_rules_are_not_better_than_claimed() {
        // 1-point rule must NOT integrate x^2 exactly.
        let rule = triangle_rule(1);
        let num: f64 = rule.iter().map(|p| p.weight * p.xi[0] * p.xi[0]).sum();
        assert!((num - triangle_monomial(2, 0)).abs() > 1e-6);
        // 3-point rule must NOT integrate x^3 exactly.
        let rule = triangle_rule(3);
        let num: f64 = rule.iter().map(|p| p.weight * p.xi[0].powi(3)).sum();
        assert!((num - triangle_monomial(3, 0)).abs() > 1e-6);
    }

    /// The exactness order claimed for the **tetrahedron** rules.
    #[test]
    fn tetrahedron_exactness_orders_hold() {
        for (n, deg) in [(1usize, 1u32), (4, 2), (5, 3)] {
            let rule = tetrahedron_rule(n);
            for i in 0..=deg {
                for j in 0..=(deg - i) {
                    for k in 0..=(deg - i - j) {
                        let num: f64 = rule
                            .iter()
                            .map(|p| {
                                p.weight
                                    * p.xi[0].powi(i as i32)
                                    * p.xi[1].powi(j as i32)
                                    * p.xi[2].powi(k as i32)
                            })
                            .sum();
                        let exact = tet_monomial(i, j, k);
                        assert!(
                            (num - exact).abs() < 1e-14,
                            "tet {}-pt, x^{} y^{} z^{}: {} vs {}",
                            n,
                            i,
                            j,
                            k,
                            num,
                            exact
                        );
                    }
                }
            }
        }
    }

    /// The exactness order claimed for the tensor-product rules, in each
    /// coordinate separately.
    #[test]
    fn tensor_product_exactness_orders_hold() {
        for n in 1..=3usize {
            let deg = 2 * n as u32 - 1;
            let q = quad_rule(n);
            for i in 0..=deg {
                for j in 0..=deg {
                    let num: f64 = q
                        .iter()
                        .map(|p| p.weight * p.xi[0].powi(i as i32) * p.xi[1].powi(j as i32))
                        .sum();
                    let exact = line_monomial(i) * line_monomial(j);
                    assert!(
                        (num - exact).abs() < 1e-13,
                        "quad {}x{}, x^{} y^{}: {} vs {}",
                        n,
                        n,
                        i,
                        j,
                        num,
                        exact
                    );
                }
            }
            let h = hex_rule(n);
            for i in 0..=deg {
                let num: f64 = h.iter().map(|p| p.weight * p.xi[2].powi(i as i32)).sum();
                let exact = line_monomial(i) * 2.0 * 2.0;
                assert!((num - exact).abs() < 1e-12, "hex {} z^{}", n, i);
            }
        }
    }

    /// Facet rules must integrate a constant traction over the facet exactly,
    /// which is the only property the traction assembly relies on.
    #[test]
    fn facet_rules_have_correct_measure() {
        assert!(
            (facet_rule(FacetType::Line2).iter().map(|p| p.weight).sum::<f64>() - 2.0).abs() < 1e-14
        );
        assert!(
            (facet_rule(FacetType::Line3).iter().map(|p| p.weight).sum::<f64>() - 2.0).abs() < 1e-14
        );
        assert!(
            (facet_rule(FacetType::Tri3).iter().map(|p| p.weight).sum::<f64>() - 0.5).abs() < 1e-14
        );
        assert!(
            (facet_rule(FacetType::Quad4).iter().map(|p| p.weight).sum::<f64>() - 4.0).abs() < 1e-14
        );
    }

    /// The default rules must be the ones the module table documents.
    #[test]
    fn default_rules_match_documented_point_counts() {
        assert_eq!(default_rule(ElementType::Tri3).len(), 1);
        assert_eq!(default_rule(ElementType::Tri6).len(), 6);
        assert_eq!(default_rule(ElementType::Quad4).len(), 4);
        assert_eq!(default_rule(ElementType::Tet4).len(), 1);
        assert_eq!(default_rule(ElementType::Hex8).len(), 8);
    }
}
