//! The full `A1`–`A4` semi-analytic expansion for one flux iterate.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `calc_a1234_expansionxyz.m` (function `calc_a1234_expansionxyz`).

use super::buckling;
use super::cross_sections::CrossSectionOperators;
use super::first_moment::{self, OddExpansion};
use super::geometry::{Axis, DirectionVectors, FaceTerms, NodalGeometry, NodalParams};
use super::leakage_moments;
use super::sparse::SparseMatrix;
use super::transverse_leakage;

/// The four expansion coefficient sets of the semi-analytic nodal method.
///
/// All four carry the units of the flux, neutrons cm⁻² s⁻¹. `A1` and `A3` are
/// the odd (surface) orders and carry a `*_first` variant for the low outer
/// face; `A2` and `A4` are the even (node-interior) orders.
#[derive(Debug, Clone, PartialEq)]
pub struct Expansion {
    /// `A1` — first order, from the face-continuity systems.
    pub first_order: OddExpansion,
    /// `A2` — second order, from the node-wise buckling systems.
    pub second_order: DirectionVectors,
    /// `A3` — third order, algebraic in `A1`.
    pub third_order: OddExpansion,
    /// `A4` — fourth order, algebraic in `A2`.
    pub fourth_order: DirectionVectors,
}

/// Computes `A1`–`A4` for the given flux and eigenvalue —
/// `calc_a1234_expansionxyz.m`.
///
/// The order is fixed by data dependence and is preserved exactly:
///
/// 1. the zeroth transverse-leakage moment, from the current flux and the
///    *previous* iteration's nodal correction terms;
/// 2. the buckling operators at the current `k_eff`;
/// 3. the first and second leakage moments;
/// 4. `A2`, from a direct sparse solve of `diag(E)*Buck + 3I`;
/// 5. `A4 = B * (Buck*A2 + L2)`;
/// 6. `A1`, via [`first_moment::assemble`];
/// 7. `A3 = A * (Buck*A1 + L1)`, and the same for the `*_first` variants.
///
/// `flux` is the current scalar flux \[neutrons cm⁻² s⁻¹\], `k_eff` the current
/// eigenvalue \[dimensionless\], `diffusion` the flat diffusion-coefficient
/// state vector \[cm\].
///
/// # The `1e6` guard, recorded
///
/// Before forming the transverse source the MATLAB replaces every zero
/// diffusion coefficient with `1e6` — a magic number whose only purpose is to
/// make the subsequent division finite (`diffvaluesDfix(diffvaluesDfix==0) =
/// 1000000; %prevent division by 0 later`). It is not a physical value and it
/// leaks into `Ssource` at out-of-core nodes, where the source is then
/// `~1e-6` times the leakage rather than zero. Reproduced verbatim.
///
/// # Panics
///
/// If the `A2` system cannot be factorised, or any input length is wrong.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    params: &NodalParams,
    geometry: &NodalGeometry,
    flux: &[f64],
    sigma: &CrossSectionOperators,
    diffusion: &[f64],
    grad_terms: &FaceTerms,
    previous_nodal_terms: &FaceTerms,
    k_eff: f64,
) -> Expansion {
    let grid = params.grid;
    let philen = params.philen();

    let leakage = transverse_leakage::zeroth_moment(
        params,
        geometry,
        flux,
        diffusion,
        grad_terms,
        previous_nodal_terms,
    );
    let buck = buckling::assemble(params, geometry, sigma, diffusion, k_eff);
    let leakage1 = leakage_moments::first_moment(params, geometry, &leakage, diffusion);
    let leakage2 = leakage_moments::second_moment(params, geometry, &leakage, diffusion);

    let coeffs = &geometry.nodal_coefficients;

    // diffvaluesDfix: zeros replaced by 1e6 to keep the divisions finite.
    let d_fix: Vec<f64> = diffusion
        .iter()
        .map(|&d| if d == 0.0 { 1_000_000.0 } else { d })
        .collect();

    let lx = geometry.width_state_vector(Axis::X, grid);
    let ly = geometry.width_state_vector(Axis::Y, grid);
    let lz = geometry.width_state_vector(Axis::Z, grid);

    let ssource = DirectionVectors {
        x: (0..philen)
            .map(|i| 0.25 * lx[i] * lx[i] * (leakage.y[i] + leakage.z[i]) / d_fix[i])
            .collect(),
        y: (0..philen)
            .map(|i| 0.25 * ly[i] * ly[i] * (leakage.x[i] + leakage.z[i]) / d_fix[i])
            .collect(),
        z: (0..philen)
            .map(|i| 0.25 * lz[i] * lz[i] * (leakage.x[i] + leakage.y[i]) / d_fix[i])
            .collect(),
    };

    // ----- A2 -----
    let identity3 = SparseMatrix::identity(philen, 3.0);
    let mut second_order = DirectionVectors::zeros(philen);
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let b = buck.axis(axis);
        let ee = coeffs.ee.axis(axis);
        let system = b.scale_rows(ee).add(&identity3);
        let bp = b.mul_vec(flux);
        let rhs: Vec<f64> = (0..philen)
            .map(|i| bp[i] - ee[i] * leakage2.axis(axis)[i] + ssource.axis(axis)[i])
            .collect();
        let lu = system
            .lu()
            .expect("the A2 system diag(E)*Buck + 3I is nonsingular");
        *second_order.axis_mut(axis) = lu.solve(&rhs);
    }

    // ----- A4 -----
    let mut fourth_order = DirectionVectors::zeros(philen);
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let ba2 = buck.axis(axis).mul_vec(second_order.axis(axis));
        let bb = coeffs.bb.axis(axis);
        let l2 = leakage2.axis(axis);
        *fourth_order.axis_mut(axis) = (0..philen).map(|i| bb[i] * (ba2[i] + l2[i])).collect();
    }

    // ----- A1 -----
    let first_order = first_moment::assemble(
        params,
        geometry,
        flux,
        &second_order,
        &fourth_order,
        &leakage1,
        diffusion,
        &buck,
    );

    // ----- A3 -----
    let mut third_order = OddExpansion::zeros(philen);
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let aa = coeffs.aa.axis(axis);
        let l1 = leakage1.axis(axis);
        let ba1 = buck.axis(axis).mul_vec(first_order.axis(axis));
        let ba1_first = buck.axis(axis).mul_vec(first_order.axis_first(axis));
        let main: Vec<f64> = (0..philen).map(|i| aa[i] * (ba1[i] + l1[i])).collect();
        let first: Vec<f64> = (0..philen)
            .map(|i| aa[i] * (ba1_first[i] + l1[i]))
            .collect();
        match axis {
            Axis::X => {
                third_order.x = main;
                third_order.x_first = first;
            }
            Axis::Y => {
                third_order.y = main;
                third_order.y_first = first;
            }
            Axis::Z => {
                third_order.z = main;
                third_order.z_first = first;
            }
        }
    }

    Expansion {
        first_order,
        second_order,
        third_order,
        fourth_order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::cross_sections::{
        assemble_operators, diffusion_coefficients, MaterialCrossSections,
    };
    use crate::reference::nodal::geometry::{BoundaryCondition, BoundaryConditions};
    use crate::reference::nodal::{gradient_diffusion, nodal_coefficients};

    /// A 2×2×3 one-group homogeneous block with reflective faces — the
    /// smallest mesh the reference's boundary blocks tolerate.
    fn setup() -> (
        NodalParams,
        NodalGeometry,
        CrossSectionOperators,
        Vec<f64>,
        FaceTerms,
    ) {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let ws = vec![1usize; n];
        let values = MaterialCrossSections {
            total: vec![vec![0.5]],
            fission: vec![vec![0.1]],
            fission_prompt: Vec::new(),
            scatter: vec![vec![vec![0.42]]],
            nu: vec![vec![2.4]],
            chi: vec![vec![1.0]],
        };
        let mut geometry = NodalGeometry::new(
            grid,
            vec![20.0; n],
            vec![20.0; n],
            vec![20.0; n],
            ws.clone(),
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        let d = diffusion_coefficients(grid, &values.total, &ws, 1.0);
        let sigma = assemble_operators(&params, &values, &ws);
        geometry.nodal_coefficients =
            nodal_coefficients::assemble(&params, &geometry, &sigma.total, &sigma.scatter, &d);
        let grad = gradient_diffusion::assemble(&params, &geometry, &d, &ws).face_terms;
        (params, geometry, sigma, d, grad)
    }

    #[test]
    fn a_flat_flux_gives_a_finite_zero_leakage_expansion() {
        let (params, geometry, sigma, d, grad) = setup();
        let philen = params.philen();
        let zero_terms = FaceTerms::zeros(philen);
        let flux = vec![1.0; philen];
        let e = assemble(
            &params,
            &geometry,
            &flux,
            &sigma,
            &d,
            &grad,
            &zero_terms,
            1.0,
        );
        for v in e
            .first_order
            .x
            .iter()
            .chain(&e.second_order.x)
            .chain(&e.third_order.x)
            .chain(&e.fourth_order.x)
        {
            assert!(v.is_finite(), "expansion must stay finite on a flat flux");
        }
        // With no transverse leakage anywhere, the even orders vanish exactly:
        // Buck*phi cancels against nothing, but the source and L2 are zero, so
        // A2 = (3I)^-1 * Buck*phi is the only surviving term.
        assert!(e.second_order.z.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn a_uniform_block_has_no_transverse_source() {
        // Reflective faces plus a spatially flat flux: every zeroth leakage
        // moment is zero, so A4 (which is B*(Buck*A2 + L2)) is driven purely by
        // A2.
        let (params, geometry, sigma, d, grad) = setup();
        let philen = params.philen();
        let zero_terms = FaceTerms::zeros(philen);
        let flux = vec![1.0; philen];
        let e = assemble(
            &params,
            &geometry,
            &flux,
            &sigma,
            &d,
            &grad,
            &zero_terms,
            1.0,
        );
        // Every node is identical, so every entry of A2 must agree.
        let first = e.second_order.z[0];
        for v in &e.second_order.z {
            assert!((v - first).abs() < 1e-12, "A2 must be uniform");
        }
    }

    #[test]
    fn the_third_order_is_algebraic_in_the_first() {
        let (params, geometry, sigma, d, grad) = setup();
        let philen = params.philen();
        let zero_terms = FaceTerms::zeros(philen);
        let grid = params.grid;
        let mut flux = vec![1.0; philen];
        for ix in 0..2 {
            for iy in 0..2 {
                flux[grid.index(0, ix, iy, 2)] = 1.5;
            }
        }
        let e = assemble(
            &params,
            &geometry,
            &flux,
            &sigma,
            &d,
            &grad,
            &zero_terms,
            1.0,
        );
        // Recompute A3 = Aa*(Buck*A1 + L1) independently for one entry, using
        // a zero L1 (flux varies only axially, and the transverse moments of a
        // reflective uniform block along z are driven by L_x + L_y = 0).
        let buck = buckling::assemble(&params, &geometry, &sigma, &d, 1.0);
        let ba1 = buck.z.mul_vec(&e.first_order.z);
        let aa = &geometry.nodal_coefficients.aa.z;
        let i = grid.index(0, 0, 0, 1);
        assert!((e.third_order.z[i] - aa[i] * ba1[i]).abs() < 1e-10);
    }
}
