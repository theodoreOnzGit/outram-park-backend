//! The `A`, `B`, `E`, `F`, `G`, `H` coefficients of the semi-analytic
//! expansion.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `calc_ABEFGHxyz.m` (functions `calc_ABEFGHxyz` and `abefgh`).

use super::geometry::{Axis, NodalCoefficients, NodalGeometry, NodalParams};
use super::sparse::SparseMatrix;

/// The six coefficients as a function of the node's optical half-width.
///
/// `alpha = 0.5 * L * sqrt(sigma_r / D)` \[dimensionless\], with `L` the node
/// width \[cm\], `sigma_r` the removal cross section \[cm⁻¹\] and `D` the
/// diffusion coefficient \[cm\]. Returned in the order `(A, B, E, F, G, H)`,
/// all dimensionless.
///
/// The expressions, verbatim from the MATLAB's inner `abefgh` function:
///
/// ```text
/// ms = 3*(cosh(a)/a - sinh(a)/a^2)
/// mc = 5*(sinh(a)/a - 3*cosh(a)/a^2 + 3*sinh(a)/a^3)
/// A  = (sinh(a) - ms) / (a^2 * ms)
/// B  = (cosh(a) - sinh(a)/a - mc) / (a^2 * mc)
/// E  = sinh(a)/(a*mc) - 3/a^2
/// F  = (a*cosh(a) - ms) / (a^2 * ms)
/// G  = (a*sinh(a) - 3*mc) / (cosh(a) - sinh(a)/a - mc)
/// H  = (a*cosh(a) - ms) / (sinh(a) - ms)
/// ```
///
/// # Valid range
///
/// `alpha` must be strictly positive and not so small that the `sinh`/`cosh`
/// cancellations lose all significance. Every expression is a ratio of
/// differences that individually vanish as `alpha -> 0`, so the accuracy
/// degrades long before `alpha` reaches zero, and at exactly `alpha = 0` the
/// result is `NaN`. **The reference has no small-`alpha` series expansion and
/// no guard.** Recorded, not fixed: adding one would change results everywhere
/// the mesh is optically thin.
#[must_use]
pub fn expansion_coefficients(alpha: f64) -> (f64, f64, f64, f64, f64, f64) {
    let a = alpha;
    let sh = a.sinh();
    let ch = a.cosh();

    let ms = 3.0 * (ch / a - sh / a / a);
    let mc = 5.0 * (sh / a - 3.0 * ch / a / a + 3.0 * sh / a.powi(3));

    let aa = (sh - ms) / (a * a * ms);
    let bb = (ch - sh / a - mc) / (a * a * mc);
    let ee = sh / a / mc - 3.0 / (a * a);
    let ff = (a * ch - ms) / (a * a * ms);
    let gg = (a * sh - 3.0 * mc) / (ch - sh / a - mc);
    let hh = (a * ch - ms) / (sh - ms);

    (aa, bb, ee, ff, gg, hh)
}

/// Builds `geometry.nodalcoeffs` — `calc_ABEFGHxyz.m`.
///
/// For each in-core node and group, forms `r = sqrt(sigma_r / D)` \[cm⁻¹\] from
/// the removal cross section on the diagonal of `sigma.tot - sigma.s`, and
/// evaluates [`expansion_coefficients`] at `0.5 * r * L` once per direction.
/// Nodes with `D == 0` — i.e. outside the core — keep all six coefficients at
/// zero.
///
/// # Divergence from MATLAB on a negative removal cross section
///
/// MATLAB's `sqrt` of a negative number returns a complex value, and the whole
/// coefficient set (and every matrix built from it downstream) silently becomes
/// complex. Rust's `f64::sqrt` returns `NaN` instead, which propagates and
/// eventually trips the solver's non-finite guard. This is a genuine
/// behavioural difference, recorded here rather than papered over; it can only
/// be reached with a data set whose within-group scattering exceeds its total
/// cross section, which is unphysical.
///
/// # Panics
///
/// If `diffusion.len()` differs from the neutronics state length.
#[must_use]
pub fn assemble(
    params: &NodalParams,
    geometry: &NodalGeometry,
    sigma_total: &SparseMatrix,
    sigma_scatter: &SparseMatrix,
    diffusion: &[f64],
) -> NodalCoefficients {
    let grid = params.grid;
    let philen = params.philen();
    assert_eq!(diffusion.len(), philen, "diffusion length");

    let removal = sigma_total.sub(sigma_scatter).diagonal();

    let lx = geometry.width_state_vector(Axis::X, grid);
    let ly = geometry.width_state_vector(Axis::Y, grid);
    let lz = geometry.width_state_vector(Axis::Z, grid);

    let mut coeffs = NodalCoefficients::zeros(philen);
    for idx in 0..philen {
        if diffusion[idx] == 0.0 {
            continue;
        }
        let r = (removal[idx] / diffusion[idx]).sqrt();
        for (axis, l) in [(Axis::X, &lx), (Axis::Y, &ly), (Axis::Z, &lz)] {
            let (aa, bb, ee, ff, gg, hh) = expansion_coefficients(0.5 * r * l[idx]);
            coeffs.aa.axis_mut(axis)[idx] = aa;
            coeffs.bb.axis_mut(axis)[idx] = bb;
            coeffs.ee.axis_mut(axis)[idx] = ee;
            coeffs.ff.axis_mut(axis)[idx] = ff;
            coeffs.gg.axis_mut(axis)[idx] = gg;
            coeffs.hh.axis_mut(axis)[idx] = hh;
        }
    }
    coeffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::geometry::{BoundaryCondition, BoundaryConditions};

    #[test]
    fn coefficients_are_finite_over_a_realistic_alpha_range() {
        // A PWR assembly node is ~20 cm wide with a fast-group removal of
        // ~0.03 cm^-1 and D ~ 1.4 cm, so alpha ~ 1.5; the thermal group can
        // reach alpha ~ 5. Sample across and either side of that band.
        for &alpha in &[0.05_f64, 0.2, 0.5, 1.0, 1.5, 3.0, 5.0, 10.0] {
            let (aa, bb, ee, ff, gg, hh) = expansion_coefficients(alpha);
            for (name, v) in [
                ("A", aa),
                ("B", bb),
                ("E", ee),
                ("F", ff),
                ("G", gg),
                ("H", hh),
            ] {
                assert!(v.is_finite(), "{name} not finite at alpha={alpha}");
            }
        }
    }

    #[test]
    fn coefficients_are_nan_at_alpha_zero_as_the_reference_leaves_them() {
        let (aa, ..) = expansion_coefficients(0.0);
        assert!(aa.is_nan(), "the reference has no small-alpha expansion");
    }

    #[test]
    fn h_matches_its_defining_ratio() {
        // H = (a*cosh a - ms)/(sinh a - ms); recompute independently.
        let a = 1.234_f64;
        let ms = 3.0 * (a.cosh() / a - a.sinh() / (a * a));
        let expected = (a * a.cosh() - ms) / (a.sinh() - ms);
        let (.., hh) = expansion_coefficients(a);
        assert!((hh - expected).abs() < 1e-14);
    }

    #[test]
    fn out_of_core_nodes_keep_zero_coefficients() {
        let grid = Grid::new(2, 1, 1, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let geometry = NodalGeometry::new(
            grid,
            vec![20.0, 20.0],
            vec![20.0, 20.0],
            vec![20.0, 20.0],
            vec![1, 0],
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let total = SparseMatrix::from_triplets(2, 2, &[(0, 0, 0.5)]);
        let scatter = SparseMatrix::from_triplets(2, 2, &[(0, 0, 0.47)]);
        let d = vec![1.4, 0.0];
        let c = assemble(&params, &geometry, &total, &scatter, &d);
        assert!(c.aa.x[0].is_finite() && c.aa.x[0] != 0.0);
        assert_eq!(c.aa.x[1], 0.0);
        assert_eq!(c.hh.z[1], 0.0);
    }

    #[test]
    fn alpha_uses_half_the_node_width_in_each_direction() {
        // A node 10 cm in x and 40 cm in z with the same sigma_r/D must give
        // exactly the coefficients of alpha and 4*alpha respectively.
        let grid = Grid::new(1, 1, 1, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0],
            vec![10.0],
            vec![40.0],
            vec![1],
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let total = SparseMatrix::from_triplets(1, 1, &[(0, 0, 0.5)]);
        let scatter = SparseMatrix::from_triplets(1, 1, &[(0, 0, 0.46)]);
        let d = vec![1.0];
        let c = assemble(&params, &geometry, &total, &scatter, &d);
        let r = 0.04_f64.sqrt();
        assert!((c.aa.x[0] - expansion_coefficients(0.5 * r * 10.0).0).abs() < 1e-15);
        assert!((c.aa.z[0] - expansion_coefficients(0.5 * r * 40.0).0).abs() < 1e-15);
    }
}
