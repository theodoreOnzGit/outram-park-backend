// SPDX-License-Identifier: AGPL-3.0-only
//! The closed-form elastic solution of a bonded stack of spherical shells
//! under inner and outer pressure, with a uniform eigenstrain `[a_r, a_θ]`
//! in each layer. It is the reference [`super::mechanics`] is verified
//! against, derived here from the same equilibrium equation.
//!
//! # Derivation
//!
//! In a layer with Lamé parameters `λ, μ` and constant eigenstrain
//! `[a_r, a_θ]`, `σ_r = (λ+2μ)(u' − a_r) + 2λ(u/r − a_θ)`,
//! `σ_θ = λ(u' − a_r) + 2(λ+μ)(u/r − a_θ)`, and
//! `σ_r − σ_θ = 2μ(u' − u/r) − 2μ(a_r − a_θ)`. Equilibrium
//! `σ_r' + 2(σ_r − σ_θ)/r = 0` becomes
//!
//! ```text
//! u'' + 2u'/r − 2u/r² = 4μ(a_r − a_θ) / ((λ + 2μ) r)
//! ```
//!
//! whose solution is `u = A r + B/r² + C r ln r` with
//! `C = 4μ(a_r − a_θ)/(3(λ + 2μ))` (substitute `C r ln r`: the left side is
//! `3C/r`). An isotropic eigenstrain gives `C = 0`, the classical Lamé form
//! of the thick-walled sphere (e.g. Timoshenko and Goodier, *Theory of
//! Elasticity*, the thick spherical shell; not re-read for this module, and
//! not needed: the derivation above is complete). Then
//!
//! ```text
//! σ_r = (3λ+2μ)A − 4μB/r³ + C[(λ+2μ)(ln r + 1) + 2λ ln r] − [(λ+2μ)a_r + 2λa_θ]
//! σ_θ = (3λ+2μ)A + 2μB/r³ + C[λ(ln r + 1) + 2(λ+μ) ln r]   − [λa_r + 2(λ+μ)a_θ]
//! ```
//!
//! The `2N` unknowns `A_i, B_i` follow from `σ_r(a) = −p_in` (or `B_1 = 0` at
//! a solid centre), continuity of `u` and `σ_r` at each interface, and
//! `σ_r(b) = −p_out`, solved by Gaussian elimination with partial pivoting.
//! `ln r` takes `r` in metres; the constant it adds is absorbed in `A`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

use super::geometry::ParticleMesh;
use super::ParticleMechanicsError;

/// The closed-form solution, one `(A, B, C)` per layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LayeredLameSolution {
    radii_m: Vec<f64>,
    coefficients: Vec<[f64; 3]>,
    lame: Vec<(f64, f64)>,
    eigenstrain: Vec<[f64; 2]>,
}

impl LayeredLameSolution {
    /// Solve for the layers of `mesh` (only their radii and elastic
    /// constants are used, the constants evaluated at `temperature_k` (K) and
    /// fast fluence `fluence_m2` (n/m²)), with eigenstrain
    /// `eigenstrain[i] = [a_r, a_θ]` in layer `i` and pressures in Pa.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidLoad`] if the eigenstrain list has the
    /// wrong length, or the innermost layer reaches the centre with an
    /// anisotropic eigenstrain (its `r ln r` stress is singular there).
    pub fn solve(
        mesh: &ParticleMesh,
        temperature_k: f64,
        fluence_m2: f64,
        eigenstrain: &[[f64; 2]],
        inner_pressure_pa: f64,
        outer_pressure_pa: f64,
    ) -> Result<Self, ParticleMechanicsError> {
        let layers = mesh.layers();
        let nl = layers.len();
        if eigenstrain.len() != nl {
            return Err(ParticleMechanicsError::InvalidLoad(format!(
                "need one eigenstrain per layer ({nl}), got {}",
                eigenstrain.len()
            )));
        }
        let mut radii_m = vec![layers[0].inner_radius_m];
        for l in layers {
            radii_m.push(l.outer_radius_m);
        }
        let lame: Vec<(f64, f64)> = layers.iter().map(|l| l.material.lame_at(temperature_k, fluence_m2)).collect();
        let cs: Vec<f64> = (0..nl)
            .map(|i| {
                let (l, m) = lame[i];
                4.0 * m * (eigenstrain[i][0] - eigenstrain[i][1]) / (3.0 * (l + 2.0 * m))
            })
            .collect();
        let solid_centre = radii_m[0] == 0.0;
        if solid_centre && cs[0] != 0.0 {
            return Err(ParticleMechanicsError::InvalidLoad(String::from(
                "an anisotropic eigenstrain in a layer reaching the centre is singular there",
            )));
        }
        let size = 2 * nl;
        let mut mat = vec![vec![0.0; size]; size];
        let mut rhs = vec![0.0; size];
        // Row helpers: u and σ_r of layer i at r, as (coef of A, coef of B, constant).
        let u_row = |i: usize, r: f64| -> (f64, f64, f64) { (r, 1.0 / (r * r), cs[i] * r * r.r_ln()) };
        let s_row = |i: usize, r: f64| -> (f64, f64, f64) {
            let (l, m) = lame[i];
            let ln = r.r_ln();
            let a = eigenstrain[i];
            (
                3.0 * l + 2.0 * m,
                -4.0 * m / (r * r * r),
                cs[i] * ((l + 2.0 * m) * (ln + 1.0) + 2.0 * l * ln) - ((l + 2.0 * m) * a[0] + 2.0 * l * a[1]),
            )
        };
        let mut row = 0;
        if solid_centre {
            mat[row][1] = 1.0;
            rhs[row] = 0.0;
        } else {
            let (ca, cb, c0) = s_row(0, radii_m[0]);
            mat[row][0] = ca;
            mat[row][1] = cb;
            rhs[row] = -inner_pressure_pa - c0;
        }
        row += 1;
        for i in 0..nl - 1 {
            let r = radii_m[i + 1];
            let (ua, ub, u0) = u_row(i, r);
            let (va, vb, v0) = u_row(i + 1, r);
            mat[row][2 * i] = ua;
            mat[row][2 * i + 1] = ub;
            mat[row][2 * i + 2] = -va;
            mat[row][2 * i + 3] = -vb;
            rhs[row] = v0 - u0;
            row += 1;
            let (sa, sb, s0) = s_row(i, r);
            let (ta, tb, t0) = s_row(i + 1, r);
            mat[row][2 * i] = sa;
            mat[row][2 * i + 1] = sb;
            mat[row][2 * i + 2] = -ta;
            mat[row][2 * i + 3] = -tb;
            rhs[row] = t0 - s0;
            row += 1;
        }
        let (ca, cb, c0) = s_row(nl - 1, radii_m[nl]);
        mat[row][2 * nl - 2] = ca;
        mat[row][2 * nl - 1] = cb;
        rhs[row] = -outer_pressure_pa - c0;
        let x = gauss_solve(mat, rhs);
        let coefficients = (0..nl).map(|i| [x[2 * i], x[2 * i + 1], cs[i]]).collect();
        Ok(Self {
            radii_m,
            coefficients,
            lame,
            eigenstrain: eigenstrain.to_vec(),
        })
    }

    fn layer_of(&self, r: f64) -> usize {
        let n = self.coefficients.len();
        match (0..n).find(|&i| r <= self.radii_m[i + 1]) {
            Some(i) => i,
            None => n - 1,
        }
    }

    /// Radial displacement at `r_m`, m.
    #[must_use]
    pub fn displacement_m(&self, r_m: f64) -> f64 {
        let i = self.layer_of(r_m);
        let [a, b, c] = self.coefficients[i];
        let r = r_m;
        if r == 0.0 {
            return 0.0;
        }
        a * r + b / (r * r) + c * r * r.r_ln()
    }

    /// Stress `[σ_r, σ_θ]` at `r_m` in layer `layer` (so an interface radius
    /// can be read from either side), Pa.
    #[must_use]
    pub fn stress_in_layer_pa(&self, layer: usize, r_m: f64) -> [f64; 2] {
        let [a, b, c] = self.coefficients[layer];
        let (l, m) = self.lame[layer];
        let e = self.eigenstrain[layer];
        let r = r_m;
        let ln = r.r_ln();
        let r3 = r * r * r;
        let (b_r, b_t) = if b == 0.0 { (0.0, 0.0) } else { (-4.0 * m * b / r3, 2.0 * m * b / r3) };
        let (c_r, c_t) = if c == 0.0 {
            (0.0, 0.0)
        } else {
            (c * ((l + 2.0 * m) * (ln + 1.0) + 2.0 * l * ln), c * (l * (ln + 1.0) + 2.0 * (l + m) * ln))
        };
        [
            (3.0 * l + 2.0 * m) * a + b_r + c_r - ((l + 2.0 * m) * e[0] + 2.0 * l * e[1]),
            (3.0 * l + 2.0 * m) * a + b_t + c_t - (l * e[0] + 2.0 * (l + m) * e[1]),
        ]
    }
}

/// Dense Gaussian elimination with partial pivoting; `mat` is square and
/// non-singular by construction (the boundary-value problem is well posed).
fn gauss_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for k in 0..n {
        let mut p = k;
        for i in k + 1..n {
            if a[i][k].abs() > a[p][k].abs() {
                p = i;
            }
        }
        a.swap(k, p);
        b.swap(k, p);
        for i in k + 1..n {
            let f = a[i][k] / a[k][k];
            if f != 0.0 {
                for j in k..n {
                    a[i][j] -= f * a[k][j];
                }
                b[i] -= f * b[k];
            }
        }
    }
    let mut x = vec![0.0; n];
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s -= a[k][j] * x[j];
        }
        x[k] = s / a[k][k];
    }
    x
}
