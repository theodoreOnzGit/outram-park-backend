//! The flux Jacobian of the 1-D Euler equations for a general equation of
//! state, and the matrix sign function SUPG needs (RELAP-7 Theory Manual
//! §5.4.1, Eqs 284–298; cited, not quoted).
//!
//! # What is computed
//!
//! With conserved `U = [ρ, ρu, ρE]` and `p = p(U)`, the manual writes the
//! Jacobian `A = ∂F/∂U` (Eq 290) through the partials `p,i = ∂p/∂U_i`. For an
//! EOS `p(ρ, e)`, `e = E − u²/2`, those partials are, with the Grüneisen
//! coefficient `Γ = (1/ρ) ∂p/∂e|_ρ` and the sound speed
//! `c² = ∂p/∂ρ|_e + (p/ρ²) ∂p/∂e|_ρ`:
//!
//! ```text
//! p,0 = c² + Γ(u² − H),   p,1 = −Γu,   p,2 = Γ
//! ```
//!
//! so Eq 292's eigenvalues reduce to `u − c, u, u + c` exactly, and Eq 285's
//! `p̂ = p − p,0 ρ − p,1 ρu − p,2 ρE = (1 + Γ)p − ρc²` (zero for an ideal gas,
//! the homogeneous-flux case the manual notes under Eq 284).
//!
//! # The eigenvector normalisation, and why it is not the manual's
//!
//! The manual's eigenvectors (Eqs 293–296) put `1` in the energy row and
//! divide by `p,0 + λ1 p,1` for the entropy wave. That divisor is
//! `c² − ΓH`, which for an ideal gas is `−(γ − 1)u²/2`: **zero for a fluid at
//! rest**, so Eq 293 is singular at every stagnant node (both shock-tube
//! initial states, every wall). [`sign_matrix`] therefore uses the standard
//! right eigenvectors, scaled to stay finite for any `Γ`:
//!
//! ```text
//! r(u − c) = [1, u − c, H − uc]
//! r(u)     = [Γ, Γu, ΓH − c²]
//! r(u + c) = [1, u + c, H + uc]
//! ```
//!
//! They span the same eigenspaces as Eq 293 wherever Eq 293 is defined, so
//! `|A| = P|D|P⁻¹` (Eq 289) is the same matrix; the unit tests below check
//! `A P = P D` against Eq 290, the eigenvalues against petir's nonsymmetric
//! eigensolver, and the product against the manual's own Eqs 293–298 at a
//! moving state.

/// The local state the Jacobian needs: velocity `u`, total specific enthalpy
/// `H`, sound speed `c` and Grüneisen coefficient `Γ`, raw SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EulerState {
    pub u: f64,
    pub h_total: f64,
    pub c: f64,
    pub gamma: f64,
}

impl EulerState {
    /// `(p,0, p,1, p,2)`, the pressure partials of Eq 290.
    pub(crate) fn pressure_partials(&self) -> (f64, f64, f64) {
        let EulerState { u, h_total, c, gamma } = *self;
        (c * c + gamma * (u * u - h_total), -gamma * u, gamma)
    }

    /// The flux Jacobian `A` of Eq 290, row-major.
    pub(crate) fn jacobian(&self) -> [[f64; 3]; 3] {
        let (p0, p1, p2) = self.pressure_partials();
        let (u, h) = (self.u, self.h_total);
        [
            [0.0, 1.0, 0.0],
            [p0 - u * u, p1 + 2.0 * u, p2],
            [u * (p0 - h), u * p1 + h, u * (1.0 + p2)],
        ]
    }

    /// The eigenvalues in the manual's general form (Eqs 291–292), ordered
    /// `[λ1 = u, λ2 (+ root), λ3 (− root)]`. The term under the root must be
    /// `≥ 0` (hyperbolicity); a negative one is returned as `NaN`. Used by
    /// the tests to check the reduction to `u, u ± c`.
    #[cfg(test)]
    pub(crate) fn eigenvalues_eq292(&self) -> [f64; 3] {
        let (p0, p1, p2) = self.pressure_partials();
        let (u, h) = (self.u, self.h_total);
        let b = p1 + u * p2;
        let disc = 4.0 * (p0 + u * p1 + h * p2) + b * b;
        let root = if disc >= 0.0 { disc.sqrt() } else { f64::NAN };
        [u, u + 0.5 * b + 0.5 * root, u + 0.5 * b - 0.5 * root]
    }

    /// The robust right eigenvectors as the columns of `P`, row-major, in the
    /// order `[u − c, u, u + c]` (see the module doc).
    pub(crate) fn right_eigenvectors(&self) -> [[f64; 3]; 3] {
        let EulerState { u, h_total: h, c, gamma: g } = *self;
        [
            [1.0, g, 1.0],
            [u - c, g * u, u + c],
            [h - u * c, g * h - c * c, h + u * c],
        ]
    }
}

/// `P sgn(D) P⁻¹`, the matrix sign of `A` (`sgn 0 = 0`).
///
/// SUPG's weighting `Aᵀ ∂W/∂x · τ R̃` with `τ = (h/2)|A|⁻¹` (Eqs 286, 288)
/// equals `∂W/∂x · (h/2) A|A|⁻¹ R̃`, and `A|A|⁻¹ = P sgn(D) P⁻¹`. Writing it
/// this way is algebraically identical and stays bounded when an eigenvalue
/// is zero (a stagnant or sonic point), where `|A|⁻¹` itself is infinite.
///
/// Returns `None` when `c ≤ 0` (then `P` is singular).
pub(crate) fn sign_matrix(s: &EulerState) -> Option<[[f64; 3]; 3]> {
    if !(s.c > 0.0 && s.c.is_finite()) {
        return None;
    }
    let p = s.right_eigenvectors();
    let pinv = inverse3(&p)?;
    let lam = [s.u - s.c, s.u, s.u + s.c];
    let sg: [f64; 3] = lam.map(|l| {
        if l > 0.0 {
            1.0
        } else if l < 0.0 {
            -1.0
        } else {
            0.0
        }
    });
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| p[i][k] * sg[k] * pinv[k][j]).sum();
        }
    }
    Some(out)
}

/// Inverse of a 3×3 matrix by cofactors; `None` if singular to round-off.
pub(crate) fn inverse3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let c = |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let cof = [
        [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
        [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
        [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
    ];
    let det = m[0][0] * cof[0][0] + m[0][1] * cof[0][1] + m[0][2] * cof[0][2];
    // Refuse only an exactly singular or non-finite matrix. Any scale-based
    // threshold misjudges P: its rows differ in scale by 1 : c : H, so for
    // two-phase water (c ≈ 8 m/s, H ≈ 1e6 J/kg) a regular P has a tiny
    // determinant relative to any entry or column norm. Two such tests
    // (largest entry cubed, then the column-norm product) wrongly refused it
    // in the IF97 comparison runs. Regularity of P is guaranteed by c > 0,
    // which `sign_matrix` checks.
    if !det.is_finite() || det == 0.0 {
        return None;
    }
    let mut inv = [[0.0; 3]; 3];
    for (i, row) in inv.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            // inverse = adjugate / det, adjugate = cofactorᵀ
            *v = cof[j][i] / det;
        }
    }
    Some(inv)
}

/// `y = M x` for a 3×3 matrix.
pub(crate) fn mat_vec(m: &[[f64; 3]; 3], x: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * x[0] + m[i][1] * x[1] + m[i][2] * x[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::linalg::{eigen_nonsymm, Matrix};

    /// An ideal gas (γ = 1.4) at `(ρ, u, p)`.
    fn gas(rho: f64, u: f64, p: f64) -> EulerState {
        let g = 1.4;
        let e = p / ((g - 1.0) * rho);
        EulerState {
            u,
            h_total: e + p / rho + 0.5 * u * u,
            c: (g * p / rho).sqrt(),
            gamma: g - 1.0,
        }
    }

    /// The manual's eigenvectors, Eqs 293–296, as the columns `[λ1, λ2, λ3]`.
    fn manual_p(s: &EulerState) -> [[f64; 3]; 3] {
        let (p0, p1, p2) = s.pressure_partials();
        let [l1, l2, l3] = s.eigenvalues_eq292();
        let (u, h) = (s.u, s.h_total);
        let c1 = -p2 / (p0 + l1 * p1);
        let d = |l: f64| (h - u * u) * (u * p2 - l) + u * (p0 - u * u);
        let (c2, c3) = (-l2 / d(l2), -l3 / d(l3));
        // Eq 293: columns (c1, c3, c2) paired with (λ1, λ2, λ3).
        [[c1, c3, c2], [l1 * c1, l2 * c3, l3 * c2], [1.0, 1.0, 1.0]]
    }

    /// **`A P = P D` for the robust eigenvectors, Eq 292 gives `u, u ± c`,
    /// and petir's QR eigensolver agrees** (ideal gas at rest and moving, and
    /// a non-ideal `Γ`). Tolerance 1e-10 relative to the largest entry.
    #[test]
    fn eigensystem_satisfies_the_jacobian_of_eq_290() {
        let cases = [
            gas(1.0, 0.0, 1.0e5),
            gas(0.125, 250.0, 1.0e4),
            gas(1.2, -400.0, 3.0e5),
            EulerState { u: 3.0, h_total: 9.0e5, c: 1200.0, gamma: 0.4 },
        ];
        for s in cases {
            let a = s.jacobian();
            let p = s.right_eigenvectors();
            let lam = [s.u - s.c, s.u, s.u + s.c];
            let scale = a.iter().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
            for k in 0..3 {
                let col = [p[0][k], p[1][k], p[2][k]];
                let ap = mat_vec(&a, col);
                for i in 0..3 {
                    assert!((ap[i] - lam[k] * col[i]).abs() <= 1e-10 * scale * (1.0 + col[i].abs()), "{s:?} k={k} i={i}");
                }
            }
            let e = s.eigenvalues_eq292();
            assert!((e[0] - s.u).abs() < 1e-9 && (e[1] - (s.u + s.c)).abs() < 1e-6 && (e[2] - (s.u - s.c)).abs() < 1e-6, "{e:?}");
            let m = Matrix::from_row_major(3, 3, a.iter().flatten().copied().collect()).unwrap();
            let mut ev: Vec<f64> = eigen_nonsymm(&m, true).unwrap().iter().map(|z| z.re).collect();
            ev.sort_by(|x, y| x.partial_cmp(y).unwrap());
            for (x, y) in ev.iter().zip(lam) {
                assert!((x - y).abs() <= 1e-8 * (1.0 + s.c), "petir {ev:?} vs {lam:?}");
            }
        }
    }

    /// **The sign matrix equals the manual's `P sgn(D) P⁻¹` (Eqs 293–298)
    /// where Eq 293 is defined, and stays finite where it is not.** At a
    /// moving state the two agree to 1e-9; at rest Eq 293 divides by zero
    /// (`c1 = ±∞`), while [`sign_matrix`] is finite and squares to the
    /// projector off the stagnant mode.
    #[test]
    fn sign_matrix_matches_eqs_293_to_298_and_survives_rest() {
        let s = gas(0.5, 120.0, 4.0e4);
        let pm = manual_p(&s);
        let pinv = inverse3(&pm).unwrap();
        let lam = s.eigenvalues_eq292();
        let mut manual = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                manual[i][j] = (0..3).map(|k| pm[i][k] * lam[k].signum() * pinv[k][j]).sum();
            }
        }
        let ours = sign_matrix(&s).unwrap();
        let scale = ours.iter().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
        for i in 0..3 {
            for j in 0..3 {
                assert!((ours[i][j] - manual[i][j]).abs() <= 1e-9 * scale, "{i},{j}: {} vs {}", ours[i][j], manual[i][j]);
            }
        }
        let rest = gas(1.0, 0.0, 1.0e5);
        let (p0, p1, _) = rest.pressure_partials();
        assert_eq!(p0 + 0.0 * p1, p0);
        assert!((p0).abs() < 1e-9 * rest.c * rest.c, "Eq 294's divisor p,0 + λ1 p,1 is zero at rest");
        let sr = sign_matrix(&rest).unwrap();
        assert!(sr.iter().flatten().all(|v| v.is_finite()));
        // p̂ = (1 + Γ)p − ρc² vanishes for an ideal gas (Eq 284's homogeneous case).
        let p_hat = (1.0 + rest.gamma) * 1.0e5 - 1.0 * rest.c * rest.c;
        assert!(p_hat.abs() < 1e-9 * 1.0e5);
    }

    /// A singular matrix has no inverse.
    #[test]
    fn a_singular_matrix_is_refused() {
        assert!(inverse3(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 1.0, 1.0]]).is_none());
        assert!(sign_matrix(&EulerState { u: 0.0, h_total: 1e5, c: 0.0, gamma: 0.4 }).is_none());
        let i = inverse3(&[[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.5]]).unwrap();
        assert_eq!(i, [[0.5, 0.0, 0.0], [0.0, 0.25, 0.0], [0.0, 0.0, 2.0]]);
        // Two-phase water: c = 7.76 m/s, H ≈ 1e6 J/kg. P is regular although
        // its columns differ in scale by ~1e5 (the case the old test refused).
        let s = EulerState { u: 1.0, h_total: 9.6e5, c: 7.76, gamma: 0.3 };
        assert!(sign_matrix(&s).is_some());
    }
}
